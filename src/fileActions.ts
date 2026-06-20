// File menu orchestration. The dialog plugin only picks paths; the actual
// read/write happens in Rust (commands.rs).

import { open, save } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { useStore } from "./store";
import type { Document, ExportFormat } from "./types";

const NATIVE_EXT = "aix";

function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

function safeName(title: string): string {
  const base = title.trim() || "Untitled";
  return base.replace(/[\\/:*?"<>|]/g, "_");
}

/** True if the active tab is an untouched blank document we can reuse. */
function activeIsPristine(): boolean {
  const s = useStore.getState();
  const c = s.doc.chunks;
  return (
    !s.dirty &&
    !s.filePath &&
    c.length === 1 &&
    !c[0].content.trim()
  );
}

/** Open a document in a new tab (or reuse the current blank one). */
function openInTab(doc: Document, filePath: string | null): void {
  if (!activeIsPristine()) useStore.getState().newTab();
  useStore.getState().loadDocument(doc, filePath);
}

export function newDocument(): void {
  // "New" opens a fresh tab (existing tabs are preserved).
  useStore.getState().newTab();
}

/**
 * Generate a fresh document draft on a theme into a new tab, streaming the
 * result into the editor in real time.
 */
export async function draftDocument(theme: string): Promise<void> {
  const s = useStore.getState();
  if (!s.hasApiKey) {
    s.notify("Set your OpenRouter API key in Settings first.", "error");
    s.openSettings();
    return;
  }
  if (!theme.trim()) return;
  // Draft into a new tab (reuse a blank one) so current work is preserved.
  if (!activeIsPristine()) useStore.getState().newTab();
  useStore.getState().setGlobalBusy("Drafting…");
  try {
    await api.aiDraftStream(theme.trim(), (e) => {
      if (e.kind === "update") {
        useStore.getState().setStreamingDocument(e.document);
      } else if (e.kind === "done") {
        useStore.getState().loadDocument(e.document, null);
      }
    });
    const n = useStore.getState().doc.chunks.length;
    useStore.getState().notify(`Draft created — ${n} chunks.`, "success");
  } catch (e) {
    useStore.getState().notify(message(e), "error");
  } finally {
    useStore.getState().setGlobalBusy(null);
  }
}

export async function importDocument(): Promise<void> {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        { name: "Text documents", extensions: ["txt", "md", "markdown", "rtf"] },
      ],
    });
    if (typeof selected !== "string") return;
    const doc = await api.importDocument(selected);
    openInTab(doc, null);
    useStore.getState().notify("Document imported.", "success");
  } catch (e) {
    useStore.getState().notify(message(e), "error");
  }
}

export async function exportDocument(format: ExportFormat): Promise<void> {
  const s = useStore.getState();
  try {
    const path = await save({
      defaultPath: `${safeName(s.doc.title)}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    await api.exportDocument(s.doc, path, format);
    s.notify(`Exported as ${format.toUpperCase()}.`, "success");
  } catch (e) {
    s.notify(message(e), "error");
  }
}

export async function openNative(): Promise<void> {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "AIX Document", extensions: [NATIVE_EXT] }],
    });
    if (typeof selected !== "string") return;
    const doc = await api.openDocumentJson(selected);
    openInTab(doc, selected);
    useStore.getState().notify("Document opened.", "success");
  } catch (e) {
    useStore.getState().notify(message(e), "error");
  }
}

export async function saveNativeAs(): Promise<void> {
  const s = useStore.getState();
  try {
    const path = await save({
      defaultPath: `${safeName(s.doc.title)}.${NATIVE_EXT}`,
      filters: [{ name: "AIX Document", extensions: [NATIVE_EXT] }],
    });
    if (!path) return;
    await api.saveDocumentJson(s.doc, path);
    s.markClean(path);
    s.notify("Document saved.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  }
}

export async function saveNative(): Promise<void> {
  const s = useStore.getState();
  if (!s.filePath) {
    await saveNativeAs();
    return;
  }
  try {
    await api.saveDocumentJson(s.doc, s.filePath);
    s.markClean();
    s.notify("Document saved.", "success");
  } catch (e) {
    s.notify(message(e), "error");
  }
}
