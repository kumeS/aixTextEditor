// Pure slide-derivation helpers shared by the Slide editor and its tests.
//
// IMPORTANT: these MUST mirror the Rust deck derivation in
// `src-tauri/src/deck.rs` (document_to_deck) and the PPTX writer in `pptx.rs`,
// so the on-screen preview/Present and the exported .pptx agree (the bug report's
// ROOT: "deck derivation implemented twice"). Each function notes the deck.rs
// rule it mirrors. Keep them in sync when either side changes.

import type { Chunk, SlideLayout } from "./types";

export interface SlideGroup {
  /** The chunks on this slide, in document order (heading first when present). */
  items: Chunk[];
  /** Each item's index in the flat document chunk list (for editing). */
  indices: number[];
}

/**
 * Group the flat chunk list into slides — a new slide begins at each heading.
 * Content before the first heading forms a leading slide with NO heading chunk
 * (its title is the document title; see `slideTitle`). Mirrors deck.rs, which
 * synthesises a doc-title heading for that leading content.
 */
export function groupSlides(chunks: Chunk[]): SlideGroup[] {
  const slides: SlideGroup[] = [];
  let cur: SlideGroup | null = null;
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

/** The heading chunk that titles this slide, if any (always items[0] when present). */
export function headingOf(s: SlideGroup): Chunk | undefined {
  return s.items.find((c) => c.metadata.chunkType === "heading");
}

/**
 * Auto-pick a layout from a slide's content. Mirrors deck.rs exactly: an image →
 * "title-image"; no body chunks → "section"; otherwise "title-content".
 * "body" = non-heading chunks (deck.rs counts the same way).
 */
export function autoLayout(items: Chunk[]): SlideLayout {
  const hasImage = items.some((c) => c.metadata.chunkType === "image");
  const body = items.filter((c) => c.metadata.chunkType !== "heading").length;
  if (hasImage) return "title-image";
  if (body === 0) return "section";
  return "title-content";
}

/**
 * The chunk that carries this slide's layout override: its heading if it has
 * one, otherwise its first chunk — so a heading-less (leading) slide can still
 * have a layout applied. Mirrors `deck.rs`, which reads the first override found.
 */
export function layoutHost(s: SlideGroup): Chunk | undefined {
  return headingOf(s) ?? s.items[0];
}

/**
 * The effective layout: an explicit override (on the heading OR, for a
 * heading-less slide, any chunk) wins; else auto-pick from content. Takes the
 * first override found so it matches `deck.rs`'s `find_map`.
 */
export function resolveLayout(s: SlideGroup): SlideLayout {
  const override = s.items.find((c) => c.metadata.layout)?.metadata.layout;
  return override ?? autoLayout(s.items);
}

/**
 * The slide title. Mirrors deck.rs: a heading slide uses the heading text; a
 * heading-less (leading) slide uses the DOCUMENT title — NOT its first paragraph
 * (which stays a bullet). This removes the old double-display where the first
 * paragraph appeared as both title and bullet (D2).
 */
export function slideTitle(s: SlideGroup, docTitle: string): string {
  const h = headingOf(s);
  return (h ? h.content : docTitle).trim();
}

/**
 * The lead chunk of a slide (heading, else first chunk) — where slide-level
 * overrides (layout, slideBody) live. Same host as `layoutHost`.
 */
export function slideLead(s: SlideGroup): Chunk | undefined {
  return headingOf(s) ?? s.items[0];
}

/**
 * The slide's explicit subtitle line, if a text chunk on it is flagged
 * `subtitle` (Req 3). Returns undefined when there's no explicit subtitle (the
 * section layout then falls back to the first bullet, matching AI Draft's
 * positional behaviour).
 */
export function slideSubtitle(s: SlideGroup): string | undefined {
  const sub = s.items.find(
    (c) => c.metadata.chunkType === "text" && c.metadata.subtitle
  );
  const text = sub?.content.trim();
  return text ? text : undefined;
}

/**
 * The slide's bullet lines. When the slide is "detached" (its lead chunk carries
 * a `slideBody`), those custom/summarised lines are used instead of the linked
 * editor paragraphs (Req 2). Otherwise every non-subtitle text chunk is a bullet
 * (deck.rs makes every non-heading paragraph a bullet; D2); an explicit subtitle
 * chunk (Req 3) is excluded since it renders in the subtitle box.
 */
export function slideBullets(s: SlideGroup): string[] {
  const override = slideLead(s)?.metadata.slideBody;
  if (override) return override.map((b) => b.trim()).filter(Boolean);
  return s.items
    .filter((c) => c.metadata.chunkType === "text" && !c.metadata.subtitle)
    .map((c) => c.content.trim())
    .filter(Boolean);
}

/** True if this slide is "detached" — showing custom slideBody, not the prose (Req 2). */
export function isSlideDetached(s: SlideGroup): boolean {
  return slideLead(s)?.metadata.slideBody !== undefined;
}

/** The slide's first image (the only one a slide layout renders), if any. */
export function slideImage(s: SlideGroup): string | undefined {
  return s.items.find((c) => c.metadata.chunkType === "image")?.content;
}

/** Diagram chunks on the slide (rendered as a placeholder; not yet in .pptx) (D3). */
export function slideDiagrams(s: SlideGroup): Chunk[] {
  return s.items.filter((c) => c.metadata.chunkType === "diagram");
}

/**
 * Per-chunk in-slide move capability (B2). A slide is a contiguous run, so moving
 * a non-boundary BODY chunk swaps it with an in-slide neighbour; the heading is
 * pinned (reorder slides via the rail) and the body can't cross the slide edges.
 * Returns whether the chunk at slide-position `pos` can move up/down within the
 * slide.
 */
export function slideMoveBounds(
  items: Chunk[],
  pos: number
): { canUp: boolean; canDown: boolean } {
  const isHeading = items[pos]?.metadata.chunkType === "heading";
  if (isHeading) return { canUp: false, canDown: false };
  const hasHeading = items[0]?.metadata.chunkType === "heading";
  const bodyStart = hasHeading ? 1 : 0;
  return {
    canUp: pos > bodyStart,
    canDown: pos < items.length - 1,
  };
}
