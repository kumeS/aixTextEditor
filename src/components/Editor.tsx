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
  const setChunkSubtitle = useStore((s) => s.setChunkSubtitle);
  const focusedChunkId = useStore((s) => s.focusedChunkId);
  const chunkIds = useStore(useShallow((s) => s.doc.chunks.map((c) => c.id)));

  // Insert after the focused chunk (else append). "Subtitle" is a text chunk
  // flagged as a subtitle, which maps to the slide's subtitle in Slide mode.
  const addSubtitle = () => {
    const id = addChunkAfter(focusedChunkId, "text");
    setChunkSubtitle(id, true);
  };

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

      <div className="mt-8 flex flex-wrap items-center gap-1.5 text-sm text-ink-faint">
        <button
          onClick={() => addChunkAfter(focusedChunkId, "text")}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 hover:bg-gray-100 hover:text-accent"
        >
          <PlusIcon /> Add paragraph
        </button>
        <button
          onClick={() => addChunkAfter(focusedChunkId, "heading")}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 hover:bg-gray-100 hover:text-accent"
          title="A heading is a slide title in Slide mode"
        >
          <PlusIcon /> Add heading
        </button>
        <button
          onClick={addSubtitle}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 hover:bg-gray-100 hover:text-accent"
          title="A subtitle sits under the title / slide title"
        >
          <PlusIcon /> Add subtitle
        </button>
      </div>
    </div>
  );
}
