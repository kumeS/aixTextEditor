// High-level AI orchestration shared by the toolbar and per-chunk menus.
// Each function manages busy state, gathers surrounding-chunk context, calls the
// Rust command, applies the result to the store, and surfaces errors as toasts.

import { api } from "./api";
import { useStore } from "./store";
import type { AiAction, Chunk } from "./types";

function neighbours(chunkId: string): {
  chunk: Chunk | undefined;
  before?: string;
  after?: string;
} {
  const chunks = useStore.getState().doc.chunks;
  const idx = chunks.findIndex((c) => c.id === chunkId);
  const chunk = chunks[idx];
  // Nearest preceding/following *text* chunk supplies context (spec §3.1).
  let before: string | undefined;
  for (let i = idx - 1; i >= 0; i--) {
    if (chunks[i].metadata.chunkType === "text" && chunks[i].content.trim()) {
      before = chunks[i].content;
      break;
    }
  }
  let after: string | undefined;
  for (let i = idx + 1; i < chunks.length; i++) {
    if (chunks[i].metadata.chunkType === "text" && chunks[i].content.trim()) {
      after = chunks[i].content;
      break;
    }
  }
  return { chunk, before, after };
}

function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Translate / proofread / summarize / custom on a single chunk. */
export async function runChunkAction(
  chunkId: string,
  action: AiAction,
  opts: { targetLanguage?: string; instruction?: string; style?: string } = {}
): Promise<void> {
  const s = useStore.getState();
  const { chunk, before, after } = neighbours(chunkId);
  if (!chunk) return;
  if (!chunk.content.trim() && action !== "custom") {
    s.notify("This paragraph is empty.", "info");
    return;
  }
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }

  s.setBusyChunk(chunkId, true);
  try {
    const result = await api.aiProcess({
      action,
      text: chunk.content,
      contextBefore: before,
      contextAfter: after,
      targetLanguage: opts.targetLanguage,
      style: opts.style,
      instruction: opts.instruction,
    });
    if (action === "summarize") {
      useStore.getState().setChunkSummary(chunkId, result);
      s.notify("Summary added to paragraph metadata.", "success");
    } else {
      useStore.getState().replaceChunkContent(chunkId, result);
      s.notify(
        action === "translate"
          ? "Translated (⌘/Ctrl+Z to undo)."
          : "Updated (⌘/Ctrl+Z to undo).",
        "success"
      );
    }
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setBusyChunk(chunkId, false);
  }
}

/** Generate a Mermaid diagram from a chunk and insert it as a new diagram chunk. */
export async function generateDiagramFromChunk(
  chunkId: string,
  instruction?: string
): Promise<void> {
  const s = useStore.getState();
  const chunk = s.doc.chunks.find((c) => c.id === chunkId);
  if (!chunk || !chunk.content.trim()) {
    s.notify("This paragraph is empty.", "info");
    return;
  }
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }

  s.setBusyChunk(chunkId, true);
  try {
    const code = await api.aiGenerateDiagram(chunk.content, instruction);
    useStore.getState().insertDiagramAfter(chunkId, code);
    s.notify("Diagram generated below the paragraph.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setBusyChunk(chunkId, false);
  }
}

/** Generate an image from one paragraph and insert it as an image chunk below. */
export async function generateImageFromChunk(chunkId: string): Promise<void> {
  const s = useStore.getState();
  const chunk = s.doc.chunks.find((c) => c.id === chunkId);
  if (!chunk || !chunk.content.trim()) {
    s.notify("This paragraph is empty.", "info");
    return;
  }
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  s.setBusyChunk(chunkId, true);
  try {
    const url = await api.aiGenerateImage(chunk.content);
    useStore.getState().insertImageAfter(chunkId, url, chunk.content.slice(0, 200));
    s.notify("Image generated below the paragraph.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setBusyChunk(chunkId, false);
  }
}

/** Generate one image from all currently selected paragraphs (combined prompt). */
export async function generateImageFromSelection(): Promise<void> {
  const s = useStore.getState();
  const ids = s.selectedChunkIds;
  if (ids.length === 0) return;
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  const selectedInOrder = s.doc.chunks.filter((c) => ids.includes(c.id));
  const prompt = selectedInOrder
    .filter(
      (c) =>
        c.metadata.chunkType === "text" || c.metadata.chunkType === "heading"
    )
    .map((c) => c.content)
    .join("\n\n")
    .trim();
  if (!prompt) {
    s.notify("Select one or more text paragraphs first.", "info");
    return;
  }
  const insertAfterId = selectedInOrder[selectedInOrder.length - 1]?.id ?? null;
  s.setGlobalBusy("Generating image…");
  try {
    const url = await api.aiGenerateImage(prompt);
    useStore.getState().insertImageAfter(insertAfterId, url, prompt.slice(0, 200));
    useStore.getState().clearSelection();
    s.notify("Image generated from selection.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}

/** Analyze the whole document and open the relationship network panel. */
export async function analyzeDocument(): Promise<void> {
  const s = useStore.getState();
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  s.setGlobalBusy("Analyzing document…");
  try {
    const result = await api.aiAnalyzeDocument(s.doc);
    // applyAnalysis persists the relationships into the document (spec §5) so
    // the graph survives a save/reopen and the document is marked dirty.
    useStore.getState().applyAnalysis(result);
    useStore.getState().toggleNetwork(true);
    if (result.nodes.length === 0) {
      s.notify("No relationships were found.", "info");
    } else {
      s.notify(
        `Found ${result.nodes.length} nodes and ${result.edges.length} relations.`,
        "success"
      );
    }
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}
