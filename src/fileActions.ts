// File menu orchestration. The dialog plugin only picks paths; the actual
// read/write happens in Rust (commands.rs).

import { open, save, confirm } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { useStore } from "./store";
import type { ExportFormat } from "./types";

const NATIVE_EXT = "aix";

function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

function safeName(title: string): string {
  const base = title.trim() || "Untitled";
  return base.replace(/[\\/:*?"<>|]/g, "_");
}

async function confirmDiscardIfDirty(): Promise<boolean> {
  const s = useStore.getState();
  if (!s.dirty) return true;
  return confirm("You have unsaved changes. Discard them?", {
    title: "Unsaved changes",
    kind: "warning",
  });
}

export async function newDocument(): Promise<void> {
  if (!(await confirmDiscardIfDirty())) return;
  try {
    const doc = await api.newDocument();
    useStore.getState().loadDocument(doc, null);
    useStore.getState().notify("New document created.", "success");
  } catch (e) {
    useStore.getState().notify(message(e), "error");
  }
}

export async function importDocument(): Promise<void> {
  if (!(await confirmDiscardIfDirty())) return;
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
    useStore.getState().loadDocument(doc, null);
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
  if (!(await confirmDiscardIfDirty())) return;
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "AIX Document", extensions: [NATIVE_EXT] }],
    });
    if (typeof selected !== "string") return;
    const doc = await api.openDocumentJson(selected);
    useStore.getState().loadDocument(doc, selected);
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
