// Slide-mode authoring surface: a thumbnail rail + a 16:9 canvas with an
// Edit / Preview / Present switch. A slide-mode document is the same chunk model
// as the editor, presented as slides — each HEADING starts a new slide and the
// chunks under it are that slide's body. Editing reuses ChunkView, so every
// per-chunk AI action, image/diagram generation and version history works inside
// slides unchanged; "Export ▸ .pptx" turns this deck into a file.
//
// WYSIWYG: thumbnails, the Preview canvas, and Present all render the SAME slide
// content at a fixed 1280×720 design size, CSS-scaled to fit — so what you see
// matches the exported layout. The tab's mode is fixed at creation.

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { bulletizeChunks } from "../aiActions";
import { useStore } from "../store";
import type { Chunk, SlideLayout } from "../types";
import ChunkView from "./ChunkView";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CopyIcon,
  PlusIcon,
  PresentIcon,
  SlidesIcon,
  SparklesIcon,
  SpinnerIcon,
  TrashIcon,
} from "./icons";

const DESIGN_W = 1280;
const DESIGN_H = 720;
const LAYOUTS: SlideLayout[] = ["section", "title-content", "title-image"];

type Slide = { items: Chunk[]; indices: number[] };

function autoLayout(items: Chunk[]): SlideLayout {
  const hasImage = items.some((c) => c.metadata.chunkType === "image");
  const body = items.filter((c) => c.metadata.chunkType !== "heading").length;
  if (hasImage) return "title-image";
  if (body === 0) return "section";
  return "title-content";
}
function headingOf(s: Slide): Chunk | undefined {
  return s.items.find((c) => c.metadata.chunkType === "heading");
}
function resolveLayout(s: Slide): SlideLayout {
  return headingOf(s)?.metadata.layout ?? autoLayout(s.items);
}
function titleOf(s: Slide): string {
  const h = headingOf(s);
  const first = h ?? s.items.find((c) => c.metadata.chunkType === "text");
  return (h?.content || first?.content || "").trim();
}
function bulletsOf(s: Slide): string[] {
  return s.items
    .filter((c) => c.metadata.chunkType === "text")
    .map((c) => c.content.trim())
    .filter(Boolean);
}
function imageOf(s: Slide): string | undefined {
  return s.items.find((c) => c.metadata.chunkType === "image")?.content;
}

/** Group the flat chunk list into slides — a new slide begins at each heading. */
function groupSlides(chunks: Chunk[]): Slide[] {
  const slides: Slide[] = [];
  let cur: Slide | null = null;
  chunks.forEach((c, i) => {
    if (c.metadata.chunkType === "heading") {
      if (cur) slides.push(cur);
      cur = { items: [c], indices: [i] };
    } else {
      if (!cur) cur = { items: [], indices: [] };
      cur.items.push(c);
      cur.indices.push(i);
    }
  });
  if (cur) slides.push(cur);
  return slides;
}

function arrayMove<T>(arr: T[], from: number, to: number): T[] {
  const next = [...arr];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
}

export default function SlideEditor() {
  const title = useStore((s) => s.doc.title);
  const setTitle = useStore((s) => s.setTitle);
  const chunks = useStore((s) => s.doc.chunks);
  const globalBusy = useStore((s) => s.globalBusy);
  const addChunkAfter = useStore((s) => s.addChunkAfter);
  const setChunkOrder = useStore((s) => s.setChunkOrder);
  const deleteChunks = useStore((s) => s.deleteChunks);
  const duplicateChunksAfter = useStore((s) => s.duplicateChunksAfter);
  const setChunkLayout = useStore((s) => s.setChunkLayout);

  const [view, setView] = useState<"edit" | "preview">("edit");
  const [anchor, setAnchor] = useState<string | null>(null);
  const [presenting, setPresenting] = useState(false);
  const [presentIdx, setPresentIdx] = useState(0);
  const dragFrom = useRef<number | null>(null);

  const slides = groupSlides(chunks);
  let selected = slides.findIndex((s) => s.items[0]?.id === anchor);
  if (selected < 0) selected = 0;
  const current: Slide | undefined = slides[selected];
  const slideIdLists = slides.map((s) => s.items.map((c) => c.id));

  // Keep the present index in range as slides change.
  const pIdx = Math.max(0, Math.min(presentIdx, slides.length - 1));
  useEffect(() => {
    if (!presenting) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === " ") {
        e.preventDefault();
        setPresentIdx((i) => Math.min(slides.length - 1, i + 1));
      } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        setPresentIdx((i) => Math.max(0, i - 1));
      } else if (e.key === "Escape") {
        setPresenting(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [presenting, slides.length]);

  const reorder = (from: number, to: number) => {
    if (from === to || to < 0 || to >= slides.length) return;
    setChunkOrder(arrayMove(slideIdLists, from, to).flat());
  };
  const addSlide = () => {
    const lastId = chunks.length ? chunks[chunks.length - 1].id : null;
    setAnchor(addChunkAfter(lastId, "heading"));
  };
  const duplicateSlide = (s: Slide) => {
    const ids = duplicateChunksAfter(s.items.map((c) => c.id));
    if (ids[0]) setAnchor(ids[0]);
  };
  const deleteSlide = (idx: number) => {
    const neighbour = slides[idx - 1] ?? slides[idx + 1];
    setAnchor(neighbour?.items[0]?.id ?? null);
    deleteChunks(slides[idx].items.map((c) => c.id));
  };
  const startPresent = () => {
    setPresentIdx(selected);
    setPresenting(true);
  };

  const currentHeading = current ? headingOf(current) : undefined;
  const currentTextIds = current
    ? current.items.filter((c) => c.metadata.chunkType === "text").map((c) => c.id)
    : [];

  return (
    <div className="flex h-full min-h-0">
      {/* ---- thumbnail rail ---- */}
      <aside className="flex w-56 shrink-0 flex-col border-r border-gray-200 bg-gray-50/60">
        <div className="flex items-center justify-between px-3 py-2 text-xs font-semibold text-ink-soft">
          <span className="flex items-center gap-1.5">
            <SlidesIcon className="h-3.5 w-3.5" /> Slides
          </span>
          <span className="text-ink-faint">{slides.length}</span>
        </div>
        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-3 pb-3">
          {slides.map((s, i) => {
            const isSel = i === selected;
            return (
              <div
                key={s.items[0]?.id ?? `s${i}`}
                draggable
                onDragStart={() => (dragFrom.current = i)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => {
                  if (dragFrom.current !== null) reorder(dragFrom.current, i);
                  dragFrom.current = null;
                }}
                className="group/thumb relative"
              >
                <button
                  onClick={() => setAnchor(s.items[0]?.id ?? null)}
                  className={`flex w-full items-stretch gap-1.5 rounded-md border p-1 text-left ${
                    isSel
                      ? "border-accent ring-1 ring-accent"
                      : "border-gray-200 hover:border-gray-300"
                  }`}
                >
                  <span className="w-4 shrink-0 pt-0.5 text-[10px] tabular-nums text-ink-faint">
                    {i + 1}
                  </span>
                  <span className="min-w-0 flex-1 overflow-hidden rounded-sm border border-gray-200">
                    <SlideStage slide={s} layout={resolveLayout(s)} />
                  </span>
                </button>
                <div
                  className={`absolute right-1 top-1 flex gap-0.5 rounded bg-white/90 p-0.5 shadow-sm transition-opacity ${
                    isSel ? "opacity-100" : "opacity-0 group-hover/thumb:opacity-100"
                  }`}
                >
                  <RailBtn title="Move up" disabled={i === 0} onClick={() => reorder(i, i - 1)}>
                    <ArrowUpIcon className="h-3 w-3" />
                  </RailBtn>
                  <RailBtn
                    title="Move down"
                    disabled={i === slides.length - 1}
                    onClick={() => reorder(i, i + 1)}
                  >
                    <ArrowDownIcon className="h-3 w-3" />
                  </RailBtn>
                  <RailBtn title="Duplicate slide" onClick={() => duplicateSlide(s)}>
                    <CopyIcon className="h-3 w-3" />
                  </RailBtn>
                  <RailBtn
                    title="Delete slide"
                    disabled={slides.length <= 1}
                    onClick={() => deleteSlide(i)}
                  >
                    <TrashIcon className="h-3 w-3" />
                  </RailBtn>
                </div>
              </div>
            );
          })}
        </div>
        <button
          onClick={addSlide}
          className="m-3 mt-0 flex items-center justify-center gap-1.5 rounded-md border border-dashed border-gray-300 py-2 text-sm text-ink-faint hover:border-accent/40 hover:text-accent"
        >
          <PlusIcon className="h-4 w-4" /> Add slide
        </button>
      </aside>

      {/* ---- canvas ---- */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-gray-100 px-6 py-2">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Untitled Deck"
            className="min-w-[8rem] flex-1 bg-transparent text-lg font-bold text-ink outline-none placeholder:text-ink-faint/40"
          />
          {/* layout picker (writes the override onto the slide's heading) */}
          <select
            value={current ? resolveLayout(current) : "title-content"}
            disabled={!currentHeading}
            onChange={(e) =>
              currentHeading &&
              setChunkLayout(currentHeading.id, e.target.value as SlideLayout)
            }
            title={currentHeading ? "Slide layout" : "Add a heading to set a layout"}
            className="rounded-md border border-gray-300 bg-white px-2 py-1 text-sm text-ink-soft disabled:opacity-40"
          >
            {LAYOUTS.map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </select>
          <button
            onClick={() => void bulletizeChunks(currentTextIds)}
            disabled={!!globalBusy || currentTextIds.length === 0}
            title="Rewrite this slide's text into bullet points (AI)"
            className="flex items-center gap-1.5 rounded-md border border-gray-300 px-2.5 py-1 text-sm text-ink-soft hover:bg-gray-100 disabled:opacity-40"
          >
            {globalBusy ? <SpinnerIcon className="h-4 w-4" /> : <SparklesIcon className="h-4 w-4" />}
            Bulletize
          </button>
          <div className="flex shrink-0 overflow-hidden rounded-md border border-gray-200 text-sm">
            {(["edit", "preview"] as const).map((m) => (
              <button
                key={m}
                onClick={() => setView(m)}
                className={`px-3 py-1 ${
                  view === m ? "bg-accent text-white" : "bg-white text-ink-soft hover:bg-gray-100"
                }`}
              >
                {m === "edit" ? "Edit" : "Preview"}
              </button>
            ))}
          </div>
          <button
            onClick={startPresent}
            title="Present full screen"
            className="flex items-center gap-1.5 rounded-md border border-gray-300 px-2.5 py-1 text-sm text-ink-soft hover:bg-gray-100"
          >
            <PresentIcon className="h-4 w-4" /> Present
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto bg-gray-100 p-6">
          <div className="mx-auto w-full max-w-[900px]">
            <div className="mb-2 flex items-center justify-between text-xs text-ink-faint">
              <span>
                Slide {slides.length ? selected + 1 : 0} / {slides.length}
              </span>
              {current && (
                <span className="rounded-full bg-accent/10 px-2 py-0.5 font-medium text-accent">
                  {resolveLayout(current)}
                </span>
              )}
            </div>
            {!current ? (
              <div className="flex aspect-[16/9] items-center justify-center rounded-lg border border-gray-300 bg-white text-sm text-ink-faint">
                No slides yet — click “Add slide”.
              </div>
            ) : view === "edit" ? (
              <div className="aspect-[16/9] w-full overflow-auto rounded-lg border border-gray-300 bg-white shadow-md">
                <div className="space-y-3 px-12 py-8">
                  {current.items.map((c, k) => (
                    <ChunkView
                      key={c.id}
                      chunkId={c.id}
                      index={current.indices[k]}
                      total={chunks.length}
                    />
                  ))}
                </div>
              </div>
            ) : (
              <div className="overflow-hidden rounded-lg border border-gray-300 shadow-md">
                <SlideStage slide={current} layout={resolveLayout(current)} />
              </div>
            )}
          </div>
        </div>
      </div>

      {/* ---- present overlay ---- */}
      {presenting && slides[pIdx] && (
        <div
          className="fixed inset-0 z-[100] flex flex-col bg-black"
          onClick={() => setPresenting(false)}
        >
          <div
            className="flex flex-1 items-center justify-center p-6"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="w-full max-w-[1280px] shadow-2xl">
              <SlideStage slide={slides[pIdx]} layout={resolveLayout(slides[pIdx])} />
            </div>
          </div>
          <div
            className="flex items-center justify-center gap-5 pb-6 text-sm text-white/70"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={() => setPresentIdx((i) => Math.max(0, i - 1))}
              disabled={pIdx === 0}
              className="hover:text-white disabled:opacity-30"
            >
              ‹ Prev
            </button>
            <span className="tabular-nums">
              {pIdx + 1} / {slides.length}
            </span>
            <button
              onClick={() => setPresentIdx((i) => Math.min(slides.length - 1, i + 1))}
              disabled={pIdx === slides.length - 1}
              className="hover:text-white disabled:opacity-30"
            >
              Next ›
            </button>
            <button onClick={() => setPresenting(false)} className="hover:text-white">
              Esc to exit
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function RailBtn({
  title,
  onClick,
  disabled,
  children,
}: {
  title: string;
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      title={title}
      aria-label={title}
      disabled={disabled}
      onClick={onClick}
      className="rounded p-0.5 text-ink-faint hover:bg-gray-100 hover:text-ink disabled:opacity-30"
    >
      {children}
    </button>
  );
}

/**
 * A slide rendered at the fixed 1280×720 design size and CSS-scaled to fill its
 * container — gives true-to-export WYSIWYG at any size (thumbnail/preview/present).
 */
function SlideStage({ slide, layout }: { slide: Slide; layout: SlideLayout }) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setScale(el.clientWidth / DESIGN_W);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <div ref={ref} className="relative aspect-[16/9] w-full overflow-hidden bg-white">
      {scale > 0 && (
        <div
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: DESIGN_W,
            height: DESIGN_H,
            transform: `scale(${scale})`,
            transformOrigin: "top left",
          }}
        >
          <SlideContent slide={slide} layout={layout} />
        </div>
      )}
    </div>
  );
}

/** The slide content at design size (1280×720). Mirrors the PPTX layouts. */
function SlideContent({ slide, layout }: { slide: Slide; layout: SlideLayout }) {
  const title = titleOf(slide) || "Untitled slide";
  const bullets = bulletsOf(slide);
  const image = imageOf(slide);
  const ink = "#1f2933";
  const soft = "#3e4c59";
  const accent = "#2563eb";

  if (layout === "section") {
    return (
      <div
        style={{ width: DESIGN_W, height: DESIGN_H, padding: 96 }}
        className="flex flex-col items-center justify-center text-center"
      >
        <div style={{ fontSize: 64, fontWeight: 700, color: ink, lineHeight: 1.15 }}>
          {title}
        </div>
      </div>
    );
  }

  const titleEl = (
    <div style={{ fontSize: 48, fontWeight: 700, color: ink, marginBottom: 36, lineHeight: 1.2 }}>
      {title}
    </div>
  );
  const bulletsEl = (
    <ul style={{ display: "flex", flexDirection: "column", gap: 18, margin: 0, padding: 0 }}>
      {bullets.map((b, i) => (
        <li key={i} style={{ display: "flex", gap: 14, fontSize: 30, lineHeight: 1.35, color: soft }}>
          <span style={{ color: accent }}>•</span>
          <span>{b}</span>
        </li>
      ))}
    </ul>
  );

  if (layout === "title-image" && image) {
    return (
      <div style={{ width: DESIGN_W, height: DESIGN_H, padding: 80 }} className="flex flex-col">
        {titleEl}
        <div style={{ display: "flex", gap: 48, flex: 1, minHeight: 0 }}>
          <div style={{ flex: "1 1 0", overflow: "hidden" }}>{bulletsEl}</div>
          <img
            src={image}
            alt=""
            style={{
              maxWidth: "45%",
              maxHeight: "100%",
              objectFit: "contain",
              alignSelf: "center",
              borderRadius: 8,
            }}
          />
        </div>
      </div>
    );
  }

  return (
    <div style={{ width: DESIGN_W, height: DESIGN_H, padding: 80 }} className="flex flex-col">
      {titleEl}
      <div style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>{bulletsEl}</div>
    </div>
  );
}
