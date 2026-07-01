import { beforeEach, describe, expect, it } from "vitest";
import { useStore } from "./store";
import type {
  AnalysisNode,
  AnalysisResult,
  Chunk,
  ChunkType,
  Document,
} from "./types";

function chunk(id: string, type: ChunkType, content = ""): Chunk {
  return { id, order: 0, content, metadata: { chunkType: type, linkedChunks: [] } };
}
function node(id: string): AnalysisNode {
  return { id, label: "", summary: "", kind: "paragraph" };
}
function doc(chunks: Chunk[], title = "T"): Document {
  return { id: "d", title, chunks, mode: "editor" };
}

/** Reset the singleton store to a known single-tab state before each test. */
function reset(chunks: Chunk[]): void {
  useStore.setState({
    tabOrder: ["tab-1"],
    activeTabId: "tab-1",
    inactiveTabs: {},
    globalBusy: null,
    busyChunks: {},
    streamingChunkId: null,
    streamingText: "",
  });
  useStore.getState().loadDocument(doc(chunks));
}

const st = () => useStore.getState();

describe("B3 — per-tab in-flight state", () => {
  it("does not bleed across tabs on switch", () => {
    reset([chunk("a", "text")]);
    const a = st().activeTabId;
    st().setGlobalBusy("Working…");
    expect(st().globalBusy).toBe("Working…");
    st().newTab();
    expect(st().globalBusy).toBeNull(); // fresh tab is clean
    expect(st().inactiveTabs[a].globalBusy).toBe("Working…"); // kept on the old tab
    st().switchTab(a);
    expect(st().globalBusy).toBe("Working…"); // restored
  });

  it("a background op's completion lands on its own tab (no stuck spinner / no foreground clear)", () => {
    reset([chunk("a", "text")]);
    const a = st().activeTabId;
    st().setGlobalBusy("Working…", a);
    st().newTab(); // a different tab is active now
    st().setGlobalBusy(null, a); // the op on `a` finishes
    expect(st().globalBusy).toBeNull(); // active tab untouched
    expect(st().inactiveTabs[a].globalBusy).toBeNull();
    st().switchTab(a);
    expect(st().globalBusy).toBeNull(); // no stuck spinner
  });

  it("routing to a closed tab is a no-op", () => {
    reset([chunk("a", "text")]);
    const a = st().activeTabId;
    st().newTab();
    st().closeTab(a);
    expect(() => st().setGlobalBusy(null, a)).not.toThrow();
    expect(st().inactiveTabs[a]).toBeUndefined();
  });
});

describe("B8 — setChunkSummary is undoable", () => {
  beforeEach(() => reset([chunk("c1", "text", "hello")]));

  it("snapshots history and round-trips through undo/redo", () => {
    st().updateChunkContent("c1", "hello!"); // make a prior edit
    const before = st().past.length;
    st().setChunkSummary("c1", "sum");
    expect(st().past.length).toBe(before + 1);
    expect(st().doc.chunks[0].metadata.summary).toBe("sum");
    st().undo();
    expect(st().doc.chunks[0].metadata.summary).toBeUndefined();
    st().redo();
    expect(st().doc.chunks[0].metadata.summary).toBe("sum");
  });

  it("does not mark the relationship graph stale", () => {
    useStore.setState({ analysisStale: false });
    st().setChunkSummary("c1", "s");
    expect(st().analysisStale).toBe(false);
  });
});

describe("A3 — analysis staleness + dangling prune", () => {
  it("text edits mark stale; metadata edits do not", () => {
    reset([chunk("c1", "text", "x"), chunk("c2", "heading", "H")]);
    useStore.setState({ analysisStale: false });
    st().updateChunkContent("c1", "y");
    expect(st().analysisStale).toBe(true);
    useStore.setState({ analysisStale: false });
    st().setHeadingLevel("c2", 3);
    expect(st().analysisStale).toBe(false);
    useStore.setState({ analysisStale: false });
    st().setChunkLayout("c2", "section");
    expect(st().analysisStale).toBe(false);
  });

  it("deleting a chunk prunes dangling nodes/edges from both graphs", () => {
    reset([chunk("c1", "text"), chunk("c2", "text")]);
    const graph: AnalysisResult = {
      nodes: [node("c1"), node("c2")],
      edges: [{ source: "c1", target: "c2", relation: "" }],
    };
    useStore.setState((s) => ({ analysis: graph, doc: { ...s.doc, analysis: graph } }));
    st().deleteChunk("c2");
    expect(st().analysis?.nodes.map((n) => n.id)).toEqual(["c1"]);
    expect(st().analysis?.edges).toEqual([]);
    expect(st().doc.analysis?.edges).toEqual([]);
  });
});

describe("B4 — undo re-derives the graph for the restored document", () => {
  it("brings the graph back in sync after undoing a structural delete", () => {
    reset([chunk("c1", "text"), chunk("c2", "text")]);
    const graph: AnalysisResult = {
      nodes: [node("c1"), node("c2")],
      edges: [{ source: "c1", target: "c2", relation: "" }],
    };
    st().applyAnalysis(graph);
    expect(st().analysis?.nodes).toHaveLength(2);
    st().deleteChunk("c2");
    expect(st().analysis?.nodes.map((n) => n.id)).toEqual(["c1"]);
    st().undo();
    expect(st().doc.chunks.map((c) => c.id)).toEqual(["c1", "c2"]);
    // The top-level graph now matches the restored doc's graph (B4).
    expect(st().analysis).toEqual(st().doc.analysis);
    expect(st().analysis?.nodes.map((n) => n.id)).toEqual(["c1", "c2"]);
  });
});

describe("B7 — duplicateChunksAfter placement", () => {
  it("inserts the copies after a contiguous block", () => {
    reset([chunk("h", "heading", "H"), chunk("a", "text", "a"), chunk("b", "text", "b")]);
    const ids = st().duplicateChunksAfter(["h", "a", "b"]);
    const order = st().doc.chunks.map((c) => c.id);
    expect(order.slice(0, 3)).toEqual(["h", "a", "b"]);
    expect(order.slice(3)).toEqual(ids);
  });

  it("inserts each copy after its own source for a non-contiguous selection", () => {
    reset([chunk("a", "text", "a"), chunk("x", "text", "x"), chunk("b", "text", "b")]);
    const ids = st().duplicateChunksAfter(["a", "b"]);
    expect(st().doc.chunks.map((c) => c.id)).toEqual(["a", ids[0], "x", "b", ids[1]]);
  });
});

describe("setMode — switch view without migrating content", () => {
  it("flips doc.mode, keeps the chunks, and no-ops when unchanged", () => {
    reset([chunk("h", "heading", "H"), chunk("a", "text", "a")]);
    expect(st().doc.mode).toBe("editor");
    st().setMode("slide");
    expect(st().doc.mode).toBe("slide");
    expect(st().doc.chunks.map((c) => c.id)).toEqual(["h", "a"]); // same chunks
    expect(st().dirty).toBe(true);
    const docRef = st().doc;
    st().setMode("slide"); // already slide → no state change
    expect(st().doc).toBe(docRef);
    st().setMode("editor");
    expect(st().doc.mode).toBe("editor");
  });
});

describe("A2 — hydrateSession", () => {
  it("restores every tab with the active one live and the rest as snapshots", () => {
    reset([chunk("a", "text")]);
    const docA = doc([chunk("a", "text", "A")], "A");
    const docB = doc([chunk("b", "text", "B")], "B");
    st().hydrateSession(
      [
        { id: "t1", doc: docA, filePath: "/a.aix", dirty: false },
        { id: "t2", doc: docB, filePath: null, dirty: true },
      ],
      "t2"
    );
    expect(st().activeTabId).toBe("t2");
    expect(st().doc).toBe(docB);
    expect(st().tabOrder).toEqual(["t1", "t2"]);
    expect(Object.keys(st().inactiveTabs)).toEqual(["t1"]);
    expect(st().inactiveTabs["t1"].doc).toBe(docA);
    st().switchTab("t1");
    expect(st().doc).toBe(docA);
    expect(st().filePath).toBe("/a.aix");
  });
});
