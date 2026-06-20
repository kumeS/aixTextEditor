// Global toolbar: file operations, export menu, undo/redo, document analysis,
// and settings. Kept visually quiet to honour the "Clarity & Simplicity" goal.

import { useEffect, useRef, useState } from "react";
import { analyzeDocument } from "../aiActions";
import {
  draftDocument,
  exportDocument,
  importDocument,
  newDocument,
  openNative,
  saveNative,
} from "../fileActions";
import { useStore } from "../store";
import type { ExportFormat } from "../types";
import { promptDialog } from "./PromptModal";
import {
  DraftIcon,
  ExportIcon,
  FileIcon,
  FolderIcon,
  ImportIcon,
  NetworkIcon,
  SaveIcon,
  SettingsIcon,
  SpinnerIcon,
} from "./icons";

function ToolButton({
  onClick,
  title,
  children,
  disabled,
}: {
  onClick: () => void;
  title: string;
  children: React.ReactNode;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      disabled={disabled}
      className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm text-ink-soft transition-colors hover:bg-gray-100 hover:text-ink disabled:opacity-40 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}

export default function Toolbar() {
  const dirty = useStore((s) => s.dirty);
  const filePath = useStore((s) => s.filePath);
  const globalBusy = useStore((s) => s.globalBusy);
  const hasApiKey = useStore((s) => s.hasApiKey);
  const model = useStore((s) => s.settings?.model ?? "");
  const canUndo = useStore((s) => s.past.length > 0);
  const canRedo = useStore((s) => s.future.length > 0);
  const undo = useStore((s) => s.undo);
  const redo = useStore((s) => s.redo);
  const openSettings = useStore((s) => s.openSettings);
  const toggleNetwork = useStore((s) => s.toggleNetwork);
  const networkOpen = useStore((s) => s.networkOpen);

  const [exportOpen, setExportOpen] = useState(false);
  const exportRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!exportOpen) return;
    const onDown = (e: MouseEvent) => {
      if (exportRef.current && !exportRef.current.contains(e.target as Node)) {
        setExportOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [exportOpen]);

  const fileName = filePath ? filePath.split(/[\\/]/).pop() : "Untitled";

  const doExport = (fmt: ExportFormat) => {
    setExportOpen(false);
    void exportDocument(fmt);
  };

  const onDraft = async () => {
    const theme = await promptDialog({
      title: "Draft a document",
      label: "What theme / topic should I draft about?",
      placeholder: "e.g. The role of attention mechanisms in NLP",
      multiline: true,
      submitLabel: "Draft",
    });
    if (theme && theme.trim()) void draftDocument(theme);
  };

  return (
    <header className="sticky top-0 z-20 flex items-center gap-1 border-b border-gray-200 bg-white/90 px-3 py-1.5 backdrop-blur">
      <div className="flex items-center gap-0.5">
        <ToolButton onClick={() => void newDocument()} title="New document">
          <FileIcon /> New
        </ToolButton>
        <ToolButton onClick={() => void openNative()} title="Open .aix document">
          <FolderIcon /> Open
        </ToolButton>
        <ToolButton onClick={() => void saveNative()} title="Save (⌘/Ctrl+S)">
          <SaveIcon /> Save
        </ToolButton>
        <ToolButton onClick={() => void importDocument()} title="Import .txt / .md / .rtf">
          <ImportIcon /> Import
        </ToolButton>

        <div ref={exportRef} className="relative">
          <ToolButton onClick={() => setExportOpen((v) => !v)} title="Export">
            <ExportIcon /> Export
          </ToolButton>
          {exportOpen && (
            <div className="absolute left-0 top-9 z-30 w-36 rounded-lg border border-gray-200 bg-white p-1 shadow-lg">
              {(["txt", "md", "rtf"] as ExportFormat[]).map((fmt) => (
                <button
                  key={fmt}
                  onClick={() => doExport(fmt)}
                  className="block w-full rounded px-2.5 py-1.5 text-left text-sm text-ink-soft hover:bg-gray-100"
                >
                  Export as .{fmt}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="mx-1 h-5 w-px bg-gray-200" />

      <ToolButton onClick={undo} title="Undo (⌘/Ctrl+Z)" disabled={!canUndo}>
        Undo
      </ToolButton>
      <ToolButton onClick={redo} title="Redo (⌘/Ctrl+Shift+Z)" disabled={!canRedo}>
        Redo
      </ToolButton>

      <div className="mx-1 h-5 w-px bg-gray-200" />

      <ToolButton
        onClick={() => void onDraft()}
        title="Draft a new document from a theme (AI)"
        disabled={!!globalBusy}
      >
        <DraftIcon /> Draft
      </ToolButton>

      <ToolButton
        onClick={() => {
          if (networkOpen) toggleNetwork(false);
          else void analyzeDocument();
        }}
        title="Analyze relationships → network graph"
      >
        <NetworkIcon /> {networkOpen ? "Hide graph" : "Analyze"}
      </ToolButton>

      {/* spacer */}
      <div className="flex-1" />

      {globalBusy && (
        <div className="flex items-center gap-1.5 text-sm text-accent">
          <SpinnerIcon className="text-accent" /> {globalBusy}
        </div>
      )}

      <div className="mx-2 truncate text-xs text-ink-faint" title={filePath ?? ""}>
        {fileName}
        {dirty ? " •" : ""}
      </div>

      <button
        onClick={openSettings}
        title={
          hasApiKey
            ? `Model: ${model || "(default)"}`
            : "API key not set — click to configure"
        }
        className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm text-ink-soft hover:bg-gray-100 hover:text-ink"
      >
        <span
          className={`h-2 w-2 rounded-full ${
            hasApiKey ? "bg-emerald-500" : "bg-amber-400"
          }`}
        />
        <SettingsIcon />
      </button>
    </header>
  );
}
