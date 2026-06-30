// Tab strip for managing multiple open documents at once. The active tab's
// state lives in the store's top-level fields; inactive tabs are snapshots.
//
// Each tab is created in a FIXED mode — Editor (prose) or Slide (deck) — chosen
// from the "+" menu and never converted afterwards. A small icon on each tab
// shows its mode.

import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { useStore } from "../store";
import type { DocMode } from "../types";
import { CloseIcon, FileIcon, PlusIcon, SlidesIcon } from "./icons";

function ModeIcon({ mode }: { mode: DocMode }) {
  return mode === "slide" ? (
    <SlidesIcon className="h-3.5 w-3.5 shrink-0 text-accent/80" />
  ) : (
    <FileIcon className="h-3.5 w-3.5 shrink-0 text-ink-faint" />
  );
}

export default function TabBar() {
  const tabOrder = useStore((s) => s.tabOrder);
  const activeTabId = useStore((s) => s.activeTabId);
  const activeTitle = useStore((s) => s.doc.title);
  const activeDirty = useStore((s) => s.dirty);
  const activeMode = useStore((s) => s.doc.mode ?? "editor");
  const inactiveTabs = useStore((s) => s.inactiveTabs);
  const switchTab = useStore((s) => s.switchTab);
  const closeTab = useStore((s) => s.closeTab);
  const newTab = useStore((s) => s.newTab);

  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [menuOpen]);

  const titleOf = (id: string) =>
    (id === activeTabId ? activeTitle : inactiveTabs[id]?.doc.title) || "Untitled";
  const dirtyOf = (id: string) =>
    id === activeTabId ? activeDirty : !!inactiveTabs[id]?.dirty;
  const modeOf = (id: string): DocMode =>
    (id === activeTabId ? activeMode : inactiveTabs[id]?.doc.mode ?? "editor") as DocMode;

  const onClose = async (id: string) => {
    if (dirtyOf(id)) {
      const ok = await confirm("This tab has unsaved changes. Close it?", {
        title: "Unsaved changes",
        kind: "warning",
      });
      if (!ok) return;
    }
    closeTab(id);
  };

  const create = (mode: DocMode) => {
    setMenuOpen(false);
    newTab(mode);
  };

  return (
    <div className="flex items-center gap-1 overflow-x-auto border-b border-gray-200 bg-gray-50/80 px-2 py-1">
      {tabOrder.map((id) => {
        const isActive = id === activeTabId;
        return (
          <div
            key={id}
            className={`group flex max-w-[220px] shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-sm ${
              isActive ? "bg-white text-ink shadow-sm" : "text-ink-faint hover:bg-white/70"
            }`}
          >
            <button
              onClick={() => switchTab(id)}
              className="flex items-center gap-1.5 truncate outline-none"
              title={`${titleOf(id)} — ${modeOf(id) === "slide" ? "Slides" : "Editor"}`}
            >
              <ModeIcon mode={modeOf(id)} />
              <span className="truncate">
                {titleOf(id)}
                {dirtyOf(id) ? " •" : ""}
              </span>
            </button>
            {tabOrder.length > 1 && (
              <button
                onClick={() => void onClose(id)}
                className="shrink-0 rounded p-0.5 text-ink-faint opacity-0 transition-opacity hover:bg-gray-200 hover:text-ink group-hover:opacity-100"
                aria-label="Close tab"
                title="Close tab"
              >
                <CloseIcon className="h-3 w-3" />
              </button>
            )}
          </div>
        );
      })}

      {/* New-tab mode selector: Editor doc vs Slide deck (mode is fixed at creation). */}
      <div ref={menuRef} className="relative shrink-0">
        <button
          onClick={() => setMenuOpen((v) => !v)}
          className="rounded-md p-1 text-ink-faint hover:bg-white hover:text-ink"
          title="New tab — choose Editor or Slides"
          aria-label="New tab"
        >
          <PlusIcon className="h-4 w-4" />
        </button>
        {menuOpen && (
          <div className="absolute left-0 top-8 z-30 w-48 rounded-lg border border-gray-200 bg-white p-1 shadow-lg">
            <button
              onClick={() => create("editor")}
              className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-sm text-ink-soft hover:bg-gray-100"
            >
              <FileIcon className="h-4 w-4 text-ink-faint" /> New Editor document
            </button>
            <button
              onClick={() => create("slide")}
              className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-sm text-ink-soft hover:bg-gray-100"
            >
              <SlidesIcon className="h-4 w-4 text-accent/80" /> New Slide deck
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
