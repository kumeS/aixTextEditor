// A single paragraph "chunk" — the Jupyter-cell-like editing unit (spec §3.1).
//
// Noiseless by design: an unfocused text chunk reads like plain prose; focusing
// it reveals a subtle accent rail and the gutter controls. Diagram chunks render
// inline Mermaid with an editable code area when focused.
//
// Selectors are per-chunk, so typing in one paragraph re-renders only this
// component (Phase 5 performance goal).

import { useEffect, useLayoutEffect, useRef } from "react";
import { runChunkAction } from "../aiActions";
import { useStore } from "../store";
import ChunkAiMenu from "./ChunkAiMenu";
import MermaidChunk from "./MermaidChunk";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  FlowIcon,
  PlusIcon,
  SummaryIcon,
  TrashIcon,
} from "./icons";

// Desired caret offset to apply when a chunk gains focus via keyboard nav.
const pendingCaret = new Map<string, number>();
export function setPendingCaret(id: string, offset: number) {
  pendingCaret.set(id, offset);
}

interface Props {
  chunkId: string;
  index: number;
  total: number;
}

export default function ChunkView({ chunkId, index, total }: Props) {
  const chunk = useStore((s) => s.doc.chunks.find((c) => c.id === chunkId));
  const busy = useStore((s) => !!s.busyChunks[chunkId]);
  const isFocused = useStore((s) => s.focusedChunkId === chunkId);
  const isFlashing = useStore((s) => s.flashChunkId === chunkId);

  const updateChunkContent = useStore((s) => s.updateChunkContent);
  const setFocused = useStore((s) => s.setFocused);
  const splitChunk = useStore((s) => s.splitChunk);
  const mergeWithPrevious = useStore((s) => s.mergeWithPrevious);
  const deleteChunk = useStore((s) => s.deleteChunk);
  const moveChunk = useStore((s) => s.moveChunk);
  const addChunkAfter = useStore((s) => s.addChunkAfter);
  const setChunkType = useStore((s) => s.setChunkType);

  const textRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-grow the textarea to fit its content.
  useLayoutEffect(() => {
    const el = textRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [chunk?.content, chunk?.metadata.chunkType, isFocused]);

  // Apply focus + any pending caret when this chunk becomes the focused one.
  useEffect(() => {
    if (!isFocused) return;
    const el = textRef.current;
    if (!el) return;
    if (document.activeElement !== el) el.focus();
    const caret = pendingCaret.get(chunkId);
    if (caret !== undefined) {
      const pos = Math.min(caret, el.value.length);
      el.setSelectionRange(pos, pos);
      pendingCaret.delete(chunkId);
    }
  }, [isFocused, chunkId]);

  // Scroll into view + flash when navigated from the network graph.
  useEffect(() => {
    if (isFlashing) {
      containerRef.current?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [isFlashing]);

  if (!chunk) return null;
  const isText = chunk.metadata.chunkType === "text";

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return; // don't interrupt IME composition
    const mod = e.metaKey || e.ctrlKey;
    const el = e.currentTarget;

    if (mod && e.key === "Enter" && e.shiftKey) {
      // Split this chunk at the caret.
      e.preventDefault();
      const newId = splitChunk(chunkId, el.selectionStart);
      if (newId) setPendingCaret(newId, 0);
      return;
    }
    if (mod && e.key === "Enter") {
      // Run the default one-click AI action.
      e.preventDefault();
      void runChunkAction(chunkId, "proofread");
      return;
    }
    if (
      e.key === "Backspace" &&
      el.selectionStart === 0 &&
      el.selectionEnd === 0 &&
      index > 0
    ) {
      const chunks = useStore.getState().doc.chunks;
      const prev = chunks[index - 1];
      if (prev && prev.metadata.chunkType === "text") {
        e.preventDefault();
        setPendingCaret(prev.id, prev.content.length);
        mergeWithPrevious(chunkId);
      }
    }
  };

  const gutterBtn =
    "flex h-6 w-6 items-center justify-center rounded text-ink-faint hover:bg-gray-100 hover:text-ink disabled:opacity-30 disabled:hover:bg-transparent";

  return (
    <div
      ref={containerRef}
      id={`chunk-${chunkId}`}
      data-chunk-id={chunkId}
      className={`group relative rounded-md transition-shadow ${
        isFlashing ? "ring-2 ring-accent/60" : ""
      }`}
    >
      {/* Left gutter: AI actions + focused accent rail */}
      <div className="absolute -left-11 top-0 flex flex-col items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 data-[on=true]:opacity-100"
        data-on={isFocused || busy}>
        <ChunkAiMenu chunkId={chunkId} isText={isText} busy={busy} />
      </div>
      <div
        className={`absolute -left-3 top-1 bottom-1 w-0.5 rounded-full transition-colors ${
          isFocused ? "bg-accent/70" : "bg-transparent"
        }`}
      />

      {/* Body */}
      {isText ? (
        <textarea
          ref={textRef}
          value={chunk.content}
          spellCheck
          placeholder={index === 0 ? "Start writing your first paragraph…" : "…"}
          onFocus={() => setFocused(chunkId)}
          onChange={(e) => updateChunkContent(chunkId, e.target.value)}
          onKeyDown={onKeyDown}
          rows={1}
          className="w-full resize-none overflow-hidden bg-transparent font-serif text-[1.075rem] leading-8 text-ink-soft outline-none placeholder:text-ink-faint/50"
        />
      ) : (
        <div>
          <MermaidChunk code={chunk.content} />
          <textarea
            ref={textRef}
            value={chunk.content}
            spellCheck={false}
            onFocus={() => setFocused(chunkId)}
            onChange={(e) => updateChunkContent(chunkId, e.target.value)}
            onKeyDown={onKeyDown}
            rows={1}
            className={`mt-1 w-full resize-none overflow-hidden rounded-md border bg-gray-50/70 p-2 font-mono text-xs leading-5 text-ink-soft outline-none transition-all ${
              isFocused
                ? "border-gray-200 opacity-100"
                : "border-transparent opacity-50 hover:opacity-100"
            }`}
          />
        </div>
      )}

      {/* Summary metadata badge (set via Summarize action) */}
      {chunk.metadata.summary && (
        <div className="mt-1 flex items-start gap-1.5 text-xs text-ink-faint">
          <SummaryIcon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span className="italic">{chunk.metadata.summary}</span>
        </div>
      )}

      {/* Right gutter: structural controls */}
      <div
        className="absolute -right-11 top-0 flex flex-col gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 data-[on=true]:opacity-100"
        data-on={isFocused}
      >
        <button
          className={gutterBtn}
          title="Add paragraph below"
          onClick={() => addChunkAfter(chunkId, "text")}
        >
          <PlusIcon />
        </button>
        <button
          className={gutterBtn}
          title="Move up"
          disabled={index === 0}
          onClick={() => moveChunk(chunkId, -1)}
        >
          <ArrowUpIcon />
        </button>
        <button
          className={gutterBtn}
          title="Move down"
          disabled={index === total - 1}
          onClick={() => moveChunk(chunkId, 1)}
        >
          <ArrowDownIcon />
        </button>
        <button
          className={gutterBtn}
          title={isText ? "Convert to diagram" : "Convert to text"}
          onClick={() => setChunkType(chunkId, isText ? "diagram" : "text")}
        >
          <FlowIcon />
        </button>
        <button
          className={`${gutterBtn} hover:text-red-500`}
          title="Delete paragraph"
          onClick={() => deleteChunk(chunkId)}
        >
          <TrashIcon />
        </button>
      </div>
    </div>
  );
}
