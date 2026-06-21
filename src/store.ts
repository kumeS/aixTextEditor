// Central application state (Zustand).
//
// Performance note (spec §4.1 / Phase 5): chunk mutations are immutable and
// preserve the object identity of *unchanged* chunks. Combined with per-chunk
// selectors in the components, editing one paragraph re-renders only that
// paragraph — not the whole document.

import { create } from "zustand";
import type {
  AnalysisResult,
  Chunk,
  ChunkType,
  Document,
  Settings,
} from "./types";

let idCounter = 0;
/** Local id generator for chunks created on the frontend. */
function localId(): string {
  idCounter += 1;
  return `c-${Date.now().toString(36)}-${idCounter}`;
}

function emptyChunk(order: number, type: ChunkType = "text"): Chunk {
  return {
    id: localId(),
    order,
    content: "",
    metadata: {
      chunkType: type,
      format: type === "diagram" ? "mermaid" : undefined,
      level: type === "heading" ? 2 : undefined,
      linkedChunks: [],
    },
  };
}

/** Reassign sequential `order` values after structural edits. */
function reindex(chunks: Chunk[]): Chunk[] {
  return chunks.map((c, i) => (c.order === i ? c : { ...c, order: i }));
}

/**
 * Rebuild the relationship graph from persisted chunk metadata (spec §5
 * `linkedChunks` + `summary`) so a saved analysis survives reopen. Returns null
 * when the document carries no relationship data. Node labels/edge relations are
 * approximate (they aren't stored verbatim), but the structure and the
 * click-to-jump targets are exact.
 */
function rebuildAnalysis(doc: Document): AnalysisResult | null {
  const textChunks = doc.chunks.filter((c) => c.metadata.chunkType === "text");
  // Only reconstruct when real relationships were persisted. A document that
  // merely has per-paragraph summaries (but was never analyzed) should not
  // resurrect a meaningless edge-less graph.
  const hasRelations = textChunks.some(
    (c) => (c.metadata.linkedChunks?.length ?? 0) > 0
  );
  if (!hasRelations) return null;

  const ids = new Set(doc.chunks.map((c) => c.id));
  const firstWords = (s: string) =>
    s.trim().split(/\s+/).slice(0, 6).join(" ");
  const nodes: AnalysisResult["nodes"] = textChunks.map((c) => {
    const summary = c.metadata.summary ?? "";
    return {
      id: c.id,
      label: firstWords(summary || c.content) || "·",
      summary,
      kind: "paragraph" as const,
    };
  });
  const edges: AnalysisResult["edges"] = [];
  for (const c of textChunks) {
    for (const target of c.metadata.linkedChunks ?? []) {
      if (ids.has(target)) edges.push({ source: c.id, target, relation: "" });
    }
  }
  return { nodes, edges };
}

export type ToastKind = "info" | "success" | "error";
export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

/**
 * Per-document state captured when a tab is backgrounded. The ACTIVE tab's
 * state lives in the top-level fields below; inactive tabs are stored as these
 * snapshots. This set must mirror EXACTLY the per-document fields (so nothing
 * leaks across tabs — e.g. filePath, or a save would overwrite another tab's file).
 */
interface TabSnapshot {
  doc: Document;
  filePath: string | null;
  dirty: boolean;
  past: Document[];
  future: Document[];
  analysis: AnalysisResult | null;
  focusedChunkId: string | null;
  lastEditChunkId: string | null;
}

function snapshotActive(s: AppState): TabSnapshot {
  return {
    doc: s.doc,
    filePath: s.filePath,
    dirty: s.dirty,
    past: s.past,
    future: s.future,
    analysis: s.analysis,
    focusedChunkId: s.focusedChunkId,
    lastEditChunkId: s.lastEditChunkId,
  };
}

function applySnapshot(snap: TabSnapshot) {
  return {
    doc: snap.doc,
    filePath: snap.filePath,
    dirty: snap.dirty,
    past: snap.past,
    future: snap.future,
    analysis: snap.analysis,
    focusedChunkId: snap.focusedChunkId,
    lastEditChunkId: snap.lastEditChunkId,
    flashChunkId: null,
    selectedChunkIds: [],
    lastAiEditChunkId: null,
  };
}

const INITIAL_TAB_ID = "tab-1";

interface AppState {
  // ----- active tab's document state (the live fields) -----
  doc: Document;
  filePath: string | null; // current native (.aix) file, if any
  dirty: boolean;

  // ----- tabs -----
  tabOrder: string[];
  activeTabId: string;
  inactiveTabs: Record<string, TabSnapshot>;

  settings: Settings | null;
  hasApiKey: boolean;

  focusedChunkId: string | null;
  flashChunkId: string | null; // transient highlight target (network-graph jump)
  selectedChunkIds: string[]; // multi-select (e.g. for image generation)
  busyChunks: Record<string, boolean>;
  globalBusy: string | null; // label of an in-flight global operation
  // Live streaming of a per-chunk AI action (translate/proofread/…): the chunk's
  // real content is untouched until the stream finalises.
  streamingChunkId: string | null;
  streamingText: string;

  analysis: AnalysisResult | null;
  networkOpen: boolean;
  settingsOpen: boolean;
  draftOpen: boolean;
  helpOpen: boolean;

  toasts: Toast[];

  // history (undo/redo) — snapshots of the document
  past: Document[];
  future: Document[];
  lastEditChunkId: string | null;
  // The chunk most recently replaced by an AI action — drives the transient
  // "what changed" diff highlight after proofread/translate/etc.
  lastAiEditChunkId: string | null;
}

interface AppActions {
  loadDocument: (doc: Document, filePath?: string | null) => void;
  setStreamingDocument: (doc: Document) => void;
  newTab: () => void;
  switchTab: (id: string) => void;
  closeTab: (id: string) => void;
  setTitle: (title: string) => void;

  updateChunkContent: (id: string, content: string) => void;
  replaceChunkContent: (id: string, content: string) => void; // undoable (AI results)
  selectChunkVersion: (id: string, value: string) => void; // swap to a saved version
  dismissAiEdit: () => void; // clear the transient diff highlight
  setChunkSummary: (id: string, summary: string) => void;
  setChunkType: (id: string, type: ChunkType) => void;
  setHeadingLevel: (id: string, level: number) => void;
  convertToHeading: (id: string, level: number, content: string) => void;

  addChunkAfter: (id: string | null, type?: ChunkType) => string;
  insertDiagramAfter: (id: string | null, code: string) => string;
  insertImageAfter: (id: string | null, url: string, prompt: string) => string;
  splitChunk: (id: string, caret: number) => string | null;
  deleteChunk: (id: string) => void;
  mergeWithPrevious: (id: string) => string | null;
  moveChunk: (id: string, dir: -1 | 1) => void;

  setFocused: (id: string | null) => void;
  flashChunk: (id: string) => void;
  toggleSelectChunk: (id: string) => void;
  clearSelection: () => void;
  setBusyChunk: (id: string, busy: boolean) => void;
  setGlobalBusy: (label: string | null) => void;
  beginChunkStream: (id: string) => void;
  updateChunkStream: (text: string) => void;
  endChunkStream: () => void;

  setSettings: (settings: Settings) => void;
  setHasApiKey: (has: boolean) => void;
  openSettings: () => void;
  closeSettings: () => void;
  openDraft: () => void;
  closeDraft: () => void;
  openHelp: () => void;
  closeHelp: () => void;

  applyAnalysis: (result: AnalysisResult) => void;
  toggleNetwork: (open?: boolean) => void;

  notify: (message: string, kind?: ToastKind) => void;
  dismissToast: (id: number) => void;

  undo: () => void;
  redo: () => void;

  markClean: (filePath?: string | null) => void;
}

const MAX_HISTORY = 100;
/** Max saved per-chunk content versions (text revisions / image URLs). */
const VERSION_LIMIT = 20;
let toastCounter = 0;

function makeInitialDoc(): Document {
  return {
    id: localId(),
    // Empty title: the editor shows a grayed placeholder until the user types.
    title: "",
    chunks: [emptyChunk(0)],
  };
}

export const useStore = create<AppState & AppActions>((set, get) => {
  /** Apply a structural/undoable mutation, snapshotting history first. */
  const commit = (producer: (doc: Document) => Document) => {
    set((state) => {
      const snapshot = state.doc;
      const next = producer(snapshot);
      const past = [...state.past, snapshot].slice(-MAX_HISTORY);
      return { doc: next, past, future: [], dirty: true, lastEditChunkId: null };
    });
  };

  const mapChunks = (doc: Document, fn: (chunks: Chunk[]) => Chunk[]): Document => ({
    ...doc,
    chunks: fn(doc.chunks),
  });

  return {
    doc: makeInitialDoc(),
    filePath: null,
    dirty: false,
    tabOrder: [INITIAL_TAB_ID],
    activeTabId: INITIAL_TAB_ID,
    inactiveTabs: {},
    settings: null,
    hasApiKey: false,
    focusedChunkId: null,
    flashChunkId: null,
    selectedChunkIds: [],
    busyChunks: {},
    globalBusy: null,
    streamingChunkId: null,
    streamingText: "",
    analysis: null,
    networkOpen: false,
    settingsOpen: false,
    draftOpen: false,
    helpOpen: false,
    toasts: [],
    past: [],
    future: [],
    lastEditChunkId: null,
    lastAiEditChunkId: null,

    loadDocument: (doc, filePath = null) =>
      set({
        doc,
        filePath,
        dirty: false,
        past: [],
        future: [],
        // Prefer the persisted full graph (paragraph + sentence nodes); fall back
        // to reconstructing a paragraph-only graph from older linkedChunks docs.
        analysis: doc.analysis ?? rebuildAnalysis(doc),
        focusedChunkId: doc.chunks[0]?.id ?? null,
        lastEditChunkId: null,
        lastAiEditChunkId: null,
        selectedChunkIds: [],
      }),

    // Live streaming snapshot (Draft): replace the document only — no history,
    // no dirty/focus churn. Chunks carry stable position ids so React reconciles
    // in place. loadDocument() finalises the stream.
    setStreamingDocument: (doc) => set({ doc }),

    // ----- tabs: active tab lives in top-level fields; others as snapshots -----
    newTab: () =>
      set((s) => {
        const id = localId();
        const fresh = makeInitialDoc();
        return {
          inactiveTabs: { ...s.inactiveTabs, [s.activeTabId]: snapshotActive(s) },
          tabOrder: [...s.tabOrder, id],
          activeTabId: id,
          doc: fresh,
          filePath: null,
          dirty: false,
          past: [],
          future: [],
          analysis: null,
          focusedChunkId: fresh.chunks[0]?.id ?? null,
          lastEditChunkId: null,
          lastAiEditChunkId: null,
          flashChunkId: null,
          selectedChunkIds: [],
        };
      }),

    switchTab: (id) =>
      set((s) => {
        if (id === s.activeTabId) return {};
        const target = s.inactiveTabs[id];
        if (!target) return {};
        const inactiveTabs = {
          ...s.inactiveTabs,
          [s.activeTabId]: snapshotActive(s),
        };
        delete inactiveTabs[id];
        return { activeTabId: id, inactiveTabs, ...applySnapshot(target) };
      }),

    closeTab: (id) =>
      set((s) => {
        if (s.tabOrder.length <= 1) return {}; // always keep one tab open
        const idx = s.tabOrder.indexOf(id);
        const order = s.tabOrder.filter((t) => t !== id);
        if (id !== s.activeTabId) {
          const inactiveTabs = { ...s.inactiveTabs };
          delete inactiveTabs[id];
          return { tabOrder: order, inactiveTabs };
        }
        // Closing the active tab: activate a neighbour (its snapshot).
        const neighbourId = order[Math.min(idx, order.length - 1)];
        const target = s.inactiveTabs[neighbourId];
        const inactiveTabs = { ...s.inactiveTabs };
        delete inactiveTabs[neighbourId];
        return {
          tabOrder: order,
          activeTabId: neighbourId,
          inactiveTabs,
          ...(target ? applySnapshot(target) : {}),
        };
      }),

    setTitle: (title) =>
      set((s) => ({ doc: { ...s.doc, title }, dirty: true })),

    // Live typing: coalesce into one undo step per continuous edit session on a
    // chunk. Replaces only the edited chunk object (others keep identity).
    updateChunkContent: (id, content) =>
      set((state) => {
        const startNewUndoStep =
          state.past.length === 0 || state.lastEditChunkId !== id;
        const past = startNewUndoStep
          ? [...state.past, state.doc].slice(-MAX_HISTORY)
          : state.past;
        return {
          doc: mapChunks(state.doc, (chunks) =>
            chunks.map((c) => (c.id === id ? { ...c, content } : c))
          ),
          past,
          future: [],
          dirty: true,
          lastEditChunkId: id,
          // Manual typing dismisses any pending AI-change highlight.
          lastAiEditChunkId: null,
        };
      }),

    // Replace a chunk's content (AI result). Saves the displaced value into the
    // chunk's `contentHistory` so the previous version can be swapped back, and
    // flags the chunk for the transient "what changed" highlight.
    replaceChunkContent: (id, content) =>
      set((state) => {
        const snapshot = state.doc;
        const next = mapChunks(snapshot, (chunks) =>
          chunks.map((c) => {
            if (c.id !== id) return c;
            const old = c.content;
            const hist = c.metadata.contentHistory ?? [];
            const nextHist =
              old.trim() && hist[hist.length - 1] !== old
                ? [...hist, old].slice(-VERSION_LIMIT)
                : hist;
            return {
              ...c,
              content,
              metadata: { ...c.metadata, contentHistory: nextHist },
            };
          })
        );
        const past = [...state.past, snapshot].slice(-MAX_HISTORY);
        return {
          doc: next,
          past,
          future: [],
          dirty: true,
          lastEditChunkId: null,
          lastAiEditChunkId: id,
        };
      }),

    // Swap a chunk to a saved version, keeping the displaced current value
    // reachable in history (so swaps are reversible).
    selectChunkVersion: (id, value) =>
      set((state) => {
        const snapshot = state.doc;
        const next = mapChunks(snapshot, (chunks) =>
          chunks.map((c) => {
            if (c.id !== id || c.content === value) return c;
            const cur = c.content;
            const hist = c.metadata.contentHistory ?? [];
            const nextHist =
              cur.trim() && !hist.includes(cur)
                ? [...hist, cur].slice(-VERSION_LIMIT)
                : hist;
            return {
              ...c,
              content: value,
              metadata: { ...c.metadata, contentHistory: nextHist },
            };
          })
        );
        const past = [...state.past, snapshot].slice(-MAX_HISTORY);
        return { doc: next, past, future: [], dirty: true, lastEditChunkId: null };
      }),

    dismissAiEdit: () => set({ lastAiEditChunkId: null }),

    setChunkSummary: (id, summary) =>
      // The summary is part of the persisted document model (spec §5), so this
      // must mark the document dirty (otherwise the save/discard guard never
      // fires and the summary is silently lost). Clearing `future` also stops a
      // pending redo from clobbering the freshly-written summary.
      set((state) => ({
        doc: mapChunks(state.doc, (chunks) =>
          chunks.map((c) =>
            c.id === id
              ? { ...c, metadata: { ...c.metadata, summary } }
              : c
          )
        ),
        dirty: true,
        future: [],
      })),

    setChunkType: (id, type) =>
      commit((doc) =>
        mapChunks(doc, (chunks) =>
          chunks.map((c) =>
            c.id === id
              ? {
                  ...c,
                  metadata: {
                    ...c.metadata,
                    chunkType: type,
                    format:
                      type === "diagram"
                        ? c.metadata.format ?? "mermaid"
                        : undefined,
                    level: type === "heading" ? c.metadata.level ?? 2 : undefined,
                  },
                }
              : c
          )
        )
      ),

    setHeadingLevel: (id, level) =>
      commit((doc) =>
        mapChunks(doc, (chunks) =>
          chunks.map((c) =>
            c.id === id
              ? {
                  ...c,
                  metadata: { ...c.metadata, chunkType: "heading", level },
                }
              : c
          )
        )
      ),

    convertToHeading: (id, level, content) =>
      commit((doc) =>
        mapChunks(doc, (chunks) =>
          chunks.map((c) =>
            c.id === id
              ? {
                  ...c,
                  content,
                  metadata: {
                    ...c.metadata,
                    chunkType: "heading",
                    level: Math.min(Math.max(level, 1), 3),
                    format: undefined,
                  },
                }
              : c
          )
        )
      ),

    addChunkAfter: (id, type = "text") => {
      const newChunk = emptyChunk(0, type);
      commit((doc) =>
        mapChunks(doc, (chunks) => {
          const found = id ? chunks.findIndex((c) => c.id === id) : -1;
          const idx = found >= 0 ? found : chunks.length - 1;
          const next = [...chunks];
          next.splice(idx + 1, 0, newChunk);
          return reindex(next);
        })
      );
      set({ focusedChunkId: newChunk.id });
      return newChunk.id;
    },

    insertDiagramAfter: (id, code) => {
      const newChunk: Chunk = { ...emptyChunk(0, "diagram"), content: code };
      commit((doc) =>
        mapChunks(doc, (chunks) => {
          const found = id ? chunks.findIndex((c) => c.id === id) : -1;
          const idx = found >= 0 ? found : chunks.length - 1;
          const next = [...chunks];
          next.splice(idx + 1, 0, newChunk);
          return reindex(next);
        })
      );
      set({ focusedChunkId: newChunk.id });
      return newChunk.id;
    },

    insertImageAfter: (id, url, prompt) => {
      const full = prompt.trim();
      const newChunk: Chunk = {
        id: localId(),
        order: 0,
        content: url,
        metadata: {
          chunkType: "image",
          summary: full ? full.slice(0, 200) : undefined,
          // Keep the full prompt so the image can be regenerated, and start an
          // empty version history (alternatives accumulate here).
          imagePrompt: full || undefined,
          linkedChunks: [],
          contentHistory: [],
        },
      };
      commit((doc) =>
        mapChunks(doc, (chunks) => {
          const found = id ? chunks.findIndex((c) => c.id === id) : -1;
          const idx = found >= 0 ? found : chunks.length - 1;
          const next = [...chunks];
          next.splice(idx + 1, 0, newChunk);
          return reindex(next);
        })
      );
      set({ focusedChunkId: newChunk.id });
      return newChunk.id;
    },

    splitChunk: (id, caret) => {
      const chunk = get().doc.chunks.find((c) => c.id === id);
      if (!chunk || chunk.metadata.chunkType !== "text") return null;
      const before = chunk.content.slice(0, caret);
      const after = chunk.content.slice(caret);
      const newChunk: Chunk = { ...emptyChunk(0), content: after };
      commit((doc) =>
        mapChunks(doc, (chunks) => {
          const idx = chunks.findIndex((c) => c.id === id);
          const next = [...chunks];
          next[idx] = { ...next[idx], content: before };
          next.splice(idx + 1, 0, newChunk);
          return reindex(next);
        })
      );
      set({ focusedChunkId: newChunk.id });
      return newChunk.id;
    },

    deleteChunk: (id) => {
      const chunks = get().doc.chunks;
      const idx = chunks.findIndex((c) => c.id === id);
      if (idx < 0) return;
      if (chunks.length <= 1) {
        // Keep one writing surface, but REUSE the id so focusedChunkId (and any
        // pending caret) stays valid and the new empty chunk keeps the caret.
        const replacement: Chunk = { ...emptyChunk(0), id };
        commit((doc) =>
          mapChunks(doc, (cs) => cs.map((c) => (c.id === id ? replacement : c)))
        );
        set({ focusedChunkId: id });
        return;
      }
      // Move focus to a sensible neighbour (previous, else next) so the caret
      // doesn't fall through to <body> after the deleted chunk unmounts.
      const neighbour = chunks[idx - 1] ?? chunks[idx + 1];
      commit((doc) =>
        mapChunks(doc, (cs) => reindex(cs.filter((c) => c.id !== id)))
      );
      set({ focusedChunkId: neighbour ? neighbour.id : null });
    },

    mergeWithPrevious: (id) => {
      const chunks = get().doc.chunks;
      const idx = chunks.findIndex((c) => c.id === id);
      if (idx <= 0) return null;
      const prev = chunks[idx - 1];
      const cur = chunks[idx];
      if (prev.metadata.chunkType !== "text" || cur.metadata.chunkType !== "text") {
        return null;
      }
      const mergedContent = prev.content + cur.content;
      const caretTarget = prev.id;
      commit((doc) =>
        mapChunks(doc, (cs) => {
          const i = cs.findIndex((c) => c.id === id);
          const next = [...cs];
          next[i - 1] = { ...next[i - 1], content: mergedContent };
          next.splice(i, 1);
          return reindex(next);
        })
      );
      set({ focusedChunkId: caretTarget });
      return caretTarget;
    },

    moveChunk: (id, dir) =>
      commit((doc) =>
        mapChunks(doc, (chunks) => {
          const idx = chunks.findIndex((c) => c.id === id);
          const target = idx + dir;
          if (idx < 0 || target < 0 || target >= chunks.length) return chunks;
          const next = [...chunks];
          [next[idx], next[target]] = [next[target], next[idx]];
          return reindex(next);
        })
      ),

    setFocused: (id) => set({ focusedChunkId: id }),
    flashChunk: (id) => {
      // Guard against stale graph nodes: after a delete/merge the persisted
      // analysis can reference a chunk id that no longer exists. Notify rather
      // than silently no-op (and don't blank whatever chunk is currently
      // focused by pointing focusedChunkId at a dead id).
      if (!get().doc.chunks.some((c) => c.id === id)) {
        get().notify(
          "That paragraph no longer exists — re-analyze to refresh the graph.",
          "info"
        );
        return;
      }
      set({ flashChunkId: id, focusedChunkId: id });
      setTimeout(() => {
        if (get().flashChunkId === id) set({ flashChunkId: null });
      }, 1600);
    },
    toggleSelectChunk: (id) =>
      set((s) => ({
        selectedChunkIds: s.selectedChunkIds.includes(id)
          ? s.selectedChunkIds.filter((x) => x !== id)
          : [...s.selectedChunkIds, id],
      })),
    clearSelection: () => set({ selectedChunkIds: [] }),
    setBusyChunk: (id, busy) =>
      set((s) => {
        const busyChunks = { ...s.busyChunks };
        if (busy) busyChunks[id] = true;
        else delete busyChunks[id];
        return { busyChunks };
      }),
    setGlobalBusy: (label) => set({ globalBusy: label }),
    beginChunkStream: (id) => set({ streamingChunkId: id, streamingText: "" }),
    updateChunkStream: (text) => set({ streamingText: text }),
    endChunkStream: () => set({ streamingChunkId: null, streamingText: "" }),

    setSettings: (settings) => set({ settings }),
    setHasApiKey: (has) => set({ hasApiKey: has }),
    openSettings: () => set({ settingsOpen: true }),
    closeSettings: () => set({ settingsOpen: false }),
    openDraft: () => set({ draftOpen: true }),
    closeDraft: () => set({ draftOpen: false }),
    openHelp: () => set({ helpOpen: true }),
    closeHelp: () => set({ helpOpen: false }),

    applyAnalysis: (result) =>
      set((state) => {
        // Persist the graph into the document model: each edge becomes a
        // `linkedChunks` entry on its source chunk, and node summaries fill in
        // `metadata.summary`. This honours spec §5 and lets the graph survive a
        // save/reopen instead of being re-fetched from the API every session.
        const linksBySource: Record<string, string[]> = {};
        for (const e of result.edges) {
          (linksBySource[e.source] ??= []);
          if (!linksBySource[e.source].includes(e.target)) {
            linksBySource[e.source].push(e.target);
          }
        }
        const summaryById: Record<string, string> = {};
        for (const n of result.nodes) {
          if (n.summary) summaryById[n.id] = n.summary;
        }
        // Chunks the analysis covered (it emits a node per text paragraph).
        const analyzed = new Set(result.nodes.map((n) => n.id));
        const chunks = state.doc.chunks.map((c) => {
          const prevLinks = c.metadata.linkedChunks ?? [];
          // Replace links wholesale for every analyzed chunk so a paragraph that
          // lost all its relations is CLEARED, not left with stale edges.
          // Chunks outside the analyzed set keep whatever they had.
          const nextLinks = analyzed.has(c.id)
            ? linksBySource[c.id] ?? []
            : prevLinks;
          const linksChanged =
            nextLinks.length !== prevLinks.length ||
            nextLinks.some((t, i) => t !== prevLinks[i]);
          const nextSummary = summaryById[c.id] ?? c.metadata.summary;
          const summaryChanged = nextSummary !== c.metadata.summary;
          if (!linksChanged && !summaryChanged) return c;
          return {
            ...c,
            metadata: {
              ...c.metadata,
              linkedChunks: linksChanged ? nextLinks : prevLinks,
              summary: nextSummary,
            },
          };
        });
        return {
          // Persist the full graph on the document so it survives save/reopen
          // (single source of truth; also keep linkedChunks for the §5 model).
          doc: { ...state.doc, chunks, analysis: result },
          analysis: result,
          dirty: true,
          future: [],
        };
      }),

    toggleNetwork: (open) =>
      set((s) => ({ networkOpen: open ?? !s.networkOpen })),

    notify: (message, kind = "info") =>
      set((s) => {
        toastCounter += 1;
        return { toasts: [...s.toasts, { id: toastCounter, message, kind }] };
      }),
    dismissToast: (id) =>
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

    undo: () =>
      set((state) => {
        if (state.past.length === 0) return state;
        const past = [...state.past];
        const previous = past.pop()!;
        return {
          doc: previous,
          past,
          future: [state.doc, ...state.future].slice(0, MAX_HISTORY),
          dirty: true,
          lastEditChunkId: null,
        };
      }),

    redo: () =>
      set((state) => {
        if (state.future.length === 0) return state;
        const [next, ...rest] = state.future;
        return {
          doc: next,
          past: [...state.past, state.doc].slice(-MAX_HISTORY),
          future: rest,
          dirty: true,
          lastEditChunkId: null,
        };
      }),

    markClean: (filePath) =>
      set((s) => ({
        dirty: false,
        filePath: filePath === undefined ? s.filePath : filePath,
      })),
  };
});
