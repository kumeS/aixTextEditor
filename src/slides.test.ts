import { describe, expect, it } from "vitest";
import {
  autoLayout,
  groupSlides,
  isSlideDetached,
  layoutHost,
  resolveLayout,
  slideBullets,
  slideMoveBounds,
  slideSubtitle,
  slideTitle,
} from "./slides";
import type { Chunk, ChunkType, SlideLayout } from "./types";

function chunk(id: string, type: ChunkType, content = ""): Chunk {
  return { id, order: 0, content, metadata: { chunkType: type, linkedChunks: [] } };
}

describe("groupSlides", () => {
  it("starts a new slide at each heading", () => {
    const slides = groupSlides([
      chunk("h1", "heading", "Intro"),
      chunk("a", "text", "A"),
      chunk("b", "text", "B"),
      chunk("h2", "heading", "Details"),
      chunk("c", "text", "C"),
    ]);
    expect(slides).toHaveLength(2);
    expect(slides[0].items.map((c) => c.id)).toEqual(["h1", "a", "b"]);
    expect(slides[1].items.map((c) => c.id)).toEqual(["h2", "c"]);
  });

  it("puts content before the first heading on a heading-less leading slide", () => {
    const slides = groupSlides([
      chunk("a", "text", "lonely"),
      chunk("h1", "heading", "Section"),
    ]);
    expect(slides).toHaveLength(2);
    expect(slides[0].items.some((c) => c.metadata.chunkType === "heading")).toBe(false);
  });
});

describe("autoLayout (mirrors deck.rs)", () => {
  it("section when no body, title-content with body, title-image with an image", () => {
    expect(autoLayout([chunk("h", "heading", "T")])).toBe("section");
    expect(autoLayout([chunk("h", "heading", "T"), chunk("a", "text", "x")])).toBe(
      "title-content"
    );
    expect(autoLayout([chunk("h", "heading", "T"), chunk("i", "image", "u")])).toBe(
      "title-image"
    );
  });
});

describe("slideTitle / slideBullets (D2 — no double display)", () => {
  it("a heading slide titles from the heading; its text are bullets", () => {
    const [s] = groupSlides([chunk("h", "heading", "Title"), chunk("a", "text", "one")]);
    expect(slideTitle(s, "Doc")).toBe("Title");
    expect(slideBullets(s)).toEqual(["one"]);
  });

  it("a heading-less slide titles from the DOCUMENT title; its first paragraph is a bullet, not the title", () => {
    const [s] = groupSlides([chunk("a", "text", "first"), chunk("b", "text", "second")]);
    expect(slideTitle(s, "My Deck")).toBe("My Deck");
    // first paragraph appears once — as a bullet — never duplicated as the title.
    expect(slideBullets(s)).toEqual(["first", "second"]);
  });
});

describe("layout override (Req 1 — applies to any slide)", () => {
  const withLayout = (id: string, type: ChunkType, layout: SlideLayout): Chunk => {
    const c = chunk(id, type, "x");
    c.metadata.layout = layout;
    return c;
  };

  it("resolveLayout reads an override from the heading OR a heading-less slide's first chunk", () => {
    const s1 = groupSlides([withLayout("h", "heading", "section"), chunk("a", "text", "a")])[0];
    expect(resolveLayout(s1)).toBe("section");
    // heading-less leading slide — override lives on the first text chunk.
    const s2 = groupSlides([withLayout("t", "text", "title-image"), chunk("u", "text", "u")])[0];
    expect(resolveLayout(s2)).toBe("title-image");
    // no override → auto-pick.
    const s3 = groupSlides([chunk("x", "heading", "X"), chunk("y", "text", "y")])[0];
    expect(resolveLayout(s3)).toBe("title-content");
  });

  it("layoutHost is the heading, else the first chunk", () => {
    const s1 = groupSlides([chunk("h", "heading", "H"), chunk("a", "text", "a")])[0];
    expect(layoutHost(s1)?.id).toBe("h");
    const s2 = groupSlides([chunk("t", "text", "t"), chunk("u", "text", "u")])[0];
    expect(layoutHost(s2)?.id).toBe("t");
  });
});

describe("subtitle (Req 3)", () => {
  it("an explicit subtitle chunk is the slide subtitle and is not a bullet", () => {
    const sub = chunk("s", "text", "My subtitle");
    sub.metadata.subtitle = true;
    const [slide] = groupSlides([chunk("h", "heading", "Title"), sub, chunk("b", "text", "Bullet one")]);
    expect(slideSubtitle(slide)).toBe("My subtitle");
    expect(slideBullets(slide)).toEqual(["Bullet one"]);
  });

  it("no explicit subtitle → undefined", () => {
    const [slide] = groupSlides([chunk("h", "heading", "T"), chunk("b", "text", "x")]);
    expect(slideSubtitle(slide)).toBeUndefined();
  });
});

describe("detach / slideBody (Req 2)", () => {
  it("a slideBody on the lead chunk overrides the bullets and marks the slide detached", () => {
    const h = chunk("h", "heading", "Title");
    h.metadata.slideBody = ["Sum A", "Sum B"];
    const [slide] = groupSlides([h, chunk("b", "text", "original prose")]);
    expect(isSlideDetached(slide)).toBe(true);
    expect(slideBullets(slide)).toEqual(["Sum A", "Sum B"]); // prose ignored
  });

  it("no slideBody → linked to the prose", () => {
    const [slide] = groupSlides([chunk("h", "heading", "T"), chunk("b", "text", "prose")]);
    expect(isSlideDetached(slide)).toBe(false);
    expect(slideBullets(slide)).toEqual(["prose"]);
  });

  it("a heading-less slide stores the override on its first chunk", () => {
    const t = chunk("t", "text", "lead");
    t.metadata.slideBody = ["S1"];
    const [slide] = groupSlides([t, chunk("u", "text", "more")]);
    expect(isSlideDetached(slide)).toBe(true);
    expect(slideBullets(slide)).toEqual(["S1"]);
  });
});

describe("slideMoveBounds (B2 — stay inside the slide)", () => {
  it("pins the heading and bounds the body to the slide", () => {
    const items = [
      chunk("h", "heading", "T"),
      chunk("a", "text", "a"),
      chunk("b", "text", "b"),
    ];
    expect(slideMoveBounds(items, 0)).toEqual({ canUp: false, canDown: false }); // heading
    expect(slideMoveBounds(items, 1)).toEqual({ canUp: false, canDown: true }); // first body
    expect(slideMoveBounds(items, 2)).toEqual({ canUp: true, canDown: false }); // last body
  });

  it("a heading-less slide moves all body chunks but not past the slide edges", () => {
    const items = [chunk("a", "text", "a"), chunk("b", "text", "b"), chunk("c", "text", "c")];
    expect(slideMoveBounds(items, 0)).toEqual({ canUp: false, canDown: true });
    expect(slideMoveBounds(items, 1)).toEqual({ canUp: true, canDown: true });
    expect(slideMoveBounds(items, 2)).toEqual({ canUp: true, canDown: false });
  });
});
