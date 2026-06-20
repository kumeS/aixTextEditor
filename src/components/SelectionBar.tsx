// Floating action bar shown when one or more chunks are selected (for combined
// image generation across paragraphs).

import { generateImageFromSelection } from "../aiActions";
import { useStore } from "../store";
import { ImageIcon, SpinnerIcon } from "./icons";

export default function SelectionBar() {
  const count = useStore((s) => s.selectedChunkIds.length);
  const clearSelection = useStore((s) => s.clearSelection);
  const globalBusy = useStore((s) => s.globalBusy);

  if (count === 0) return null;

  return (
    <div className="pointer-events-auto fixed bottom-5 left-1/2 z-40 flex -translate-x-1/2 items-center gap-3 rounded-full border border-gray-200 bg-white px-4 py-2 shadow-lg">
      <span className="text-sm text-ink-soft">{count} selected</span>
      <button
        onClick={() => void generateImageFromSelection()}
        disabled={!!globalBusy}
        className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-white hover:bg-accent-soft disabled:opacity-50"
      >
        {globalBusy ? (
          <SpinnerIcon className="text-white" />
        ) : (
          <ImageIcon className="h-4 w-4" />
        )}
        Generate image
      </button>
      <button
        onClick={clearSelection}
        className="text-sm text-ink-faint hover:text-ink"
      >
        Clear
      </button>
    </div>
  );
}
