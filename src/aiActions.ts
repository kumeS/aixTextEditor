// High-level AI orchestration shared by the toolbar and per-chunk menus.
// Each function manages busy state, gathers surrounding-chunk context, calls the
// Rust command, applies the result to the store, and surfaces errors as toasts.

import { api } from "./api";
import { useStore } from "./store";
import type { AiAction, Chunk } from "./types";

// T1 — whole-document context assembly. Caps keep the prompt bounded on long docs.
const DOC_MAP_MAX_LINES = 60;
const DOC_MAP_MAX_CHARS = 3000;
const LINKED_MAX_CHARS = 2500;

/**
 * Assemble the context an AI action gets for a chunk. Beyond the immediate
 * preceding/following prose (spec §3.1), this now makes the model
 * document-aware (T1): the SECTION the chunk lives under, a compact outline of
 * the whole document (headings + per-chunk summaries), and the full text of any
 * graph-linked chunks. This turns the latent summary/linkedChunks data — until
 * now computed and saved but never fed back into editing — into editing context.
 */
function gatherContext(chunkId: string): {
  chunk: Chunk | undefined;
  before?: string;
  after?: string;
  sectionHeading?: string;
  documentMap?: string;
  linkedContent?: string;
} {
  const chunks = useStore.getState().doc.chunks;
  const idx = chunks.findIndex((c) => c.id === chunkId);
  const chunk = chunks[idx];
  if (!chunk) return { chunk };

  // Nearest preceding/following *text* chunk supplies immediate context (§3.1).
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

  // The section: nearest preceding heading.
  let sectionHeading: string | undefined;
  for (let i = idx - 1; i >= 0; i--) {
    if (chunks[i].metadata.chunkType === "heading" && chunks[i].content.trim()) {
      sectionHeading = chunks[i].content.trim();
      break;
    }
  }

  // A compact whole-document outline: headings + any per-chunk summaries.
  const lines: string[] = [];
  let mapChars = 0;
  let hasContext = false;
  for (let i = 0; i < chunks.length && lines.length < DOC_MAP_MAX_LINES; i++) {
    if (i === idx) {
      lines.push("- «the paragraph you are editing»");
      continue;
    }
    const c = chunks[i];
    let line: string | null = null;
    if (c.metadata.chunkType === "heading" && c.content.trim()) {
      const lvl = "#".repeat(Math.min(3, Math.max(1, c.metadata.level ?? 1)));
      line = `${lvl} ${c.content.trim()}`;
    } else if (c.metadata.summary && c.metadata.summary.trim()) {
      line = `- ${c.metadata.summary.trim()}`;
    }
    if (line) {
      if (mapChars + line.length > DOC_MAP_MAX_CHARS) break;
      lines.push(line);
      mapChars += line.length;
      hasContext = true;
    }
  }
  const documentMap = hasContext ? lines.join("\n") : undefined;

  // Full content of graph-linked chunks (the supporting/related material).
  let linkedContent: string | undefined;
  const linked = chunk.metadata.linkedChunks ?? [];
  if (linked.length) {
    const byId = new Map(chunks.map((c) => [c.id, c]));
    const parts: string[] = [];
    let chars = 0;
    for (const id of linked) {
      const lc = byId.get(id);
      if (!lc || lc.id === chunkId || !lc.content.trim()) continue;
      const snippet = lc.content.trim().slice(0, 800);
      if (chars + snippet.length > LINKED_MAX_CHARS) break;
      parts.push(snippet);
      chars += snippet.length;
    }
    if (parts.length) linkedContent = parts.join("\n\n---\n\n");
  }

  return { chunk, before, after, sectionHeading, documentMap, linkedContent };
}

function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/** Endpoints served from the local machine (e.g. Ollama) don't need an API key. */
function isLocalEndpoint(endpoint: string | undefined): boolean {
  const e = (endpoint ?? "").toLowerCase();
  return (
    e.includes("localhost") ||
    e.includes("127.0.0.1") ||
    e.includes("0.0.0.0") ||
    e.includes("[::1]")
  );
}

/**
 * Whether AI calls can proceed: a key is set, OR the endpoint is a local
 * (keyless) provider such as Ollama. Used to gate every AI action.
 */
export function aiReady(): boolean {
  const s = useStore.getState();
  return s.hasApiKey || isLocalEndpoint(s.settings?.endpoint);
}

/**
 * True while `chunkId` still exists in the ACTIVE document — i.e. the user has
 * not switched tabs (or deleted the chunk) during an async AI call. Guards
 * against a result landing in the wrong tab when generation finishes late.
 */
function chunkStillActive(chunkId: string): boolean {
  return useStore.getState().doc.chunks.some((c) => c.id === chunkId);
}

/**
 * Slide AI: rewrite a slide's prose into concise bullet points. Takes the
 * slide's text-chunk ids, asks the model for short bullets, and replaces those
 * chunks with one text chunk per bullet (each bullet = its own slide line).
 */
export async function bulletizeChunks(ids: string[]): Promise<void> {
  const s = useStore.getState();
  if (!ids.length) return;
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  const tab = s.activeTabId;
  const idSet = new Set(ids);
  const texts = s.doc.chunks
    .filter((c) => idSet.has(c.id) && c.metadata.chunkType === "text" && c.content.trim())
    .map((c) => c.content.trim());
  if (!texts.length) {
    s.notify("This slide has no text to bulletize.", "info");
    return;
  }

  s.setGlobalBusy("Bulletizing…");
  try {
    const result = await api.aiProcess({
      action: "custom",
      text: texts.join("\n\n"),
      instruction:
        "Rewrite the text as concise presentation bullet points. Output ONLY the bullets, " +
        "one per line, each starting with '- '. Use 3 to 6 bullets, each a short phrase " +
        "(not a full sentence). Keep the meaning faithful; do not invent facts. No title, no preamble.",
      outputLanguage: s.settings?.defaultTargetLanguage,
      tone: s.settings?.writingTone || undefined,
    });
    if (useStore.getState().activeTabId !== tab) {
      s.notify("Switched tabs — bulletize discarded.", "info");
      return;
    }
    const lines = result
      .split("\n")
      .map((l) => l.replace(/^\s*[-•*]\s*/, "").trim())
      .filter(Boolean);
    if (!lines.length) {
      s.notify("The model returned no bullets.", "info");
      return;
    }
    useStore.getState().replaceChunksWithTexts(ids, lines);
    s.notify(`Bulletized into ${lines.length} points (⌘/Ctrl+Z to undo).`, "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}

/** Translate / proofread / summarize / custom on a single chunk. */
export async function runChunkAction(
  chunkId: string,
  action: AiAction,
  opts: { targetLanguage?: string; instruction?: string; style?: string } = {}
): Promise<void> {
  const s = useStore.getState();
  const tab = s.activeTabId; // B3: scope live-stream mutations to the originating tab
  const { chunk, before, after, sectionHeading, documentMap, linkedContent } =
    gatherContext(chunkId);
  if (!chunk) return;
  if (!chunk.content.trim() && action !== "custom") {
    s.notify("This paragraph is empty.", "info");
    return;
  }
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }

  const request = {
    action,
    text: chunk.content,
    contextBefore: before,
    contextAfter: after,
    targetLanguage: opts.targetLanguage,
    style: opts.style,
    instruction: opts.instruction,
    // Pin every non-translate action's output to the configured default
    // language (so e.g. proofreading Japanese text never drifts to English)
    // and apply the global writing tone.
    outputLanguage: s.settings?.defaultTargetLanguage,
    tone: s.settings?.writingTone || undefined,
    // T1: document-wide awareness.
    sectionHeading,
    documentMap,
    linkedContent,
  };

  // Stream every action except "summarize" (which writes metadata, not content).
  const streaming = action !== "summarize";
  s.setBusyChunk(chunkId, true);
  if (streaming) useStore.getState().beginChunkStream(chunkId);
  try {
    const result = streaming
      ? await api.aiProcessStream(request, (text) => {
          // Only paint while the originating tab is still active AND this is the
          // chunk being streamed — so a backgrounded op can't hijack another
          // tab's live streaming UI (B3).
          const st = useStore.getState();
          if (st.activeTabId === tab && st.streamingChunkId === chunkId) {
            st.updateChunkStream(text);
          }
        })
      : await api.aiProcess(request);
    if (!chunkStillActive(chunkId)) {
      s.notify("Switched away from that paragraph — result discarded.", "info");
      return;
    }
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
    // B3: only clear the shared stream state if we still own the active tab,
    // so finishing a backgrounded op doesn't kill the foreground tab's stream.
    if (streaming && useStore.getState().activeTabId === tab) {
      useStore.getState().endChunkStream();
    }
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
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }

  s.setBusyChunk(chunkId, true);
  try {
    const code = await api.aiGenerateDiagram(chunk.content, instruction);
    if (!chunkStillActive(chunkId)) {
      s.notify("Switched away from that paragraph — diagram discarded.", "info");
      return;
    }
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
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  s.setBusyChunk(chunkId, true);
  try {
    const url = await api.aiGenerateImage(chunk.content);
    if (!chunkStillActive(chunkId)) {
      s.notify("Switched away from that paragraph — image discarded.", "info");
      return;
    }
    useStore.getState().insertImageAfter(chunkId, url, chunk.content);
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
  if (!aiReady()) {
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
  const tab = s.activeTabId;
  s.setGlobalBusy("Generating image…");
  try {
    const url = await api.aiGenerateImage(prompt);
    if (useStore.getState().activeTabId !== tab) {
      s.notify("Switched tabs — image discarded.", "info");
      return;
    }
    useStore.getState().insertImageAfter(insertAfterId, url, prompt);
    useStore.getState().clearSelection();
    s.notify("Image generated from selection.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}

/** A presentation-style prompt wrapper: ask the image model for a clean diagram. */
function presentationPrompt(text: string): string {
  return (
    "A clean, minimal presentation slide diagram that visually explains the following content. " +
    "Use a simple flat design with clear labels, boxes and arrows, generous white space, a " +
    "restrained professional colour palette, and NO photorealism. Content to illustrate:\n\n" +
    text.trim()
  );
}

/**
 * Generate a simple presentation-style figure (a diagram-like image) from a
 * paragraph and insert it as an image chunk below. Distinct from a literal
 * image: it asks the model for an explanatory slide graphic.
 */
export async function generatePresentationFromChunk(chunkId: string): Promise<void> {
  const s = useStore.getState();
  const chunk = s.doc.chunks.find((c) => c.id === chunkId);
  if (!chunk || !chunk.content.trim()) {
    s.notify("This paragraph is empty.", "info");
    return;
  }
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  const prompt = presentationPrompt(chunk.content);
  s.setBusyChunk(chunkId, true);
  try {
    const url = await api.aiGenerateImage(prompt);
    if (!chunkStillActive(chunkId)) {
      s.notify("Switched away from that paragraph — figure discarded.", "info");
      return;
    }
    useStore.getState().insertImageAfter(chunkId, url, prompt);
    s.notify("Presentation figure generated below the paragraph.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setBusyChunk(chunkId, false);
  }
}

/**
 * Regenerate an image chunk from its stored prompt and save the result as a new
 * version in the chunk's history (the user can swap between alternatives).
 */
export async function regenerateImageChunk(chunkId: string): Promise<void> {
  const s = useStore.getState();
  const chunk = s.doc.chunks.find((c) => c.id === chunkId);
  if (!chunk || chunk.metadata.chunkType !== "image") return;
  const prompt = chunk.metadata.imagePrompt || chunk.metadata.summary || "";
  if (!prompt.trim()) {
    s.notify("No source prompt is stored for this image.", "info");
    return;
  }
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  s.setBusyChunk(chunkId, true);
  try {
    const url = await api.aiGenerateImage(prompt);
    if (!chunkStillActive(chunkId)) {
      s.notify("Switched away — regenerated image discarded.", "info");
      return;
    }
    // replaceChunkContent stores the previous URL in history, so every
    // alternative stays selectable.
    useStore.getState().replaceChunkContent(chunkId, url);
    s.notify("New image version generated.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  } finally {
    useStore.getState().setBusyChunk(chunkId, false);
  }
}

/**
 * Apply one instruction to every selected text/heading chunk at once (multi-
 * paragraph editing). Each paragraph keeps its surrounding context and its prior
 * version in history. Runs sequentially to respect provider rate limits.
 */
export async function editSelection(instruction: string): Promise<void> {
  const s = useStore.getState();
  const ids = s.selectedChunkIds;
  if (ids.length === 0) return;
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  const text = instruction.trim();
  if (!text) return;
  // Process selected chunks in document order; skip non-text chunks.
  const ordered = s.doc.chunks.filter(
    (c) =>
      ids.includes(c.id) &&
      (c.metadata.chunkType === "text" || c.metadata.chunkType === "heading") &&
      c.content.trim()
  );
  if (ordered.length === 0) {
    s.notify("Select one or more non-empty text paragraphs first.", "info");
    return;
  }
  const tab = s.activeTabId;
  s.setGlobalBusy(`Editing ${ordered.length} paragraphs…`);
  let done = 0;
  try {
    for (const c of ordered) {
      if (useStore.getState().activeTabId !== tab) break;
      await runChunkAction(c.id, "custom", { instruction: text });
      done += 1;
      useStore.getState().setGlobalBusy(`Editing ${done}/${ordered.length}…`);
    }
    if (useStore.getState().activeTabId === tab) {
      useStore.getState().clearSelection();
      s.notify(`Edited ${done} paragraph${done === 1 ? "" : "s"}.`, "success");
    }
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}

/**
 * Pick a system voice that matches the script of `text` so e.g. Japanese isn't
 * read with an English voice. Returns undefined (system default) for Latin text.
 * Rust verifies the voice is installed and falls back if not.
 */
function voiceForText(text: string): string | undefined {
  if (/[぀-ヿ]/.test(text)) return "Kyoko"; // hiragana/katakana → Japanese
  if (/[가-힣]/.test(text)) return "Yuna"; // hangul → Korean
  if (/[一-鿿]/.test(text)) return "Tingting"; // Han (no kana) → Chinese
  return undefined;
}

/** Read a paragraph aloud via the OS speech synthesizer. */
export async function speakChunk(chunkId: string): Promise<void> {
  const s = useStore.getState();
  const chunk = s.doc.chunks.find((c) => c.id === chunkId);
  if (!chunk || !chunk.content.trim()) {
    s.notify("Nothing to read here.", "info");
    return;
  }
  try {
    await api.speakText(chunk.content, voiceForText(chunk.content));
  } catch (e) {
    s.notify(message(e), "error");
  }
}

export async function stopSpeaking(): Promise<void> {
  try {
    await api.stopSpeaking();
  } catch {
    /* best-effort */
  }
}

/** Analyze the whole document and open the relationship network panel. */
export async function analyzeDocument(): Promise<void> {
  const s = useStore.getState();
  if (!aiReady()) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  const tab = s.activeTabId;
  s.setGlobalBusy("Analyzing document…");
  try {
    const result = await api.aiAnalyzeDocument(s.doc);
    if (useStore.getState().activeTabId !== tab) {
      s.notify("Switched tabs — analysis discarded.", "info");
      return;
    }
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
