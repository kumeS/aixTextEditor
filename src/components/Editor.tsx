// The writing surface: document title + the ordered list of chunks.
//
// This component only subscribes to the *list of chunk ids* (and the title), so
// it re-renders on structural changes (add/remove/reorder) — not on every
// keystroke. Each ChunkView subscribes to its own chunk.

import { useShallow } from "zustand/react/shallow";
import { useStore } from "../store";
import ChunkView from "./ChunkView";
import { PlusIcon } from "./icons";

export default function Editor() {
  const title = useStore((s) => s.doc.title);
  const setTitle = useStore((s) => s.setTitle);
  const addChunkAfter = useStore((s) => s.addChunkAfter);
  const chunkIds = useStore(useShallow((s) => s.doc.chunks.map((c) => c.id)));

  return (
    <div className="mx-auto w-full max-w-prose px-12 py-16">
      <input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="Untitled Document"
        className="mb-10 w-full bg-transparent font-sans text-4xl font-bold text-ink outline-none placeholder:text-ink-faint/40"
      />

      <div className="space-y-5">
        {chunkIds.map((id, i) => (
          <ChunkView key={id} chunkId={id} index={i} total={chunkIds.length} />
        ))}
      </div>

      <button
        onClick={() => addChunkAfter(null, "text")}
        className="mt-8 flex items-center gap-1.5 rounded-md px-2 py-1 text-sm text-ink-faint hover:bg-gray-100 hover:text-accent"
      >
        <PlusIcon /> Add paragraph
      </button>
    </div>
  );
}
