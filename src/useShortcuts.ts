// Global keyboard shortcuts (Phase 5). Per-chunk shortcuts (AI run, split,
// merge) live in ChunkView; these are the document-level ones.

import { useEffect } from "react";
import { saveNative, openNative } from "./fileActions";
import { useStore } from "./store";

export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      const key = e.key.toLowerCase();

      if (key === "s") {
        e.preventDefault();
        void saveNative();
      } else if (key === "o") {
        e.preventDefault();
        void openNative();
      } else if (key === "t") {
        e.preventDefault();
        useStore.getState().newTab();
      } else if (key === "z" && !e.shiftKey) {
        e.preventDefault();
        useStore.getState().undo();
      } else if ((key === "z" && e.shiftKey) || key === "y") {
        e.preventDefault();
        useStore.getState().redo();
      } else if (key === ",") {
        e.preventDefault();
        useStore.getState().openSettings();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
