// Floating action bar shown when one or more chunks are selected: generate one
// combined image, or apply a single edit instruction to every selected
// paragraph at once (multi-paragraph editing).

import { editSelection, generateImageFromSelection } from "../aiActions";
import { useStore } from "../store";
import { promptDialog } from "./PromptModal";
import { EditIcon, ImageIcon, SpinnerIcon } from "./icons";

export default function SelectionBar() {
  const count = useStore((s) => s.selectedChunkIds.length);
  const clearSelection = useStore((s) => s.clearSelection);
  const globalBusy = useStore((s) => s.globalBusy);

  if (count === 0) return null;

  const onEditAll = async () => {
    const instruction = await promptDialog({
      title: `Edit ${count} paragraphs`,
      label: "Describe the change to apply to every selected paragraph",
      placeholder: "e.g. Make each more concise and formal",
      multiline: true,
      submitLabel: "Apply to all",
    });
    if (instruction && instruction.trim()) void editSelection(instruction);
  };

  return (
    <div className="pointer-events-auto fixed bottom-5 left-1/2 z-40 flex -translate-x-1/2 items-center gap-3 rounded-full border border-gray-200 bg-white px-4 py-2 shadow-lg">
      <span className="text-sm text-ink-soft">{count} selected</span>
      <button
        onClick={() => void onEditAll()}
        disabled={!!globalBusy}
        className="flex items-center gap-1.5 rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium text-ink-soft hover:bg-gray-100 disabled:opacity-50"
      >
        <EditIcon className="h-4 w-4" /> Edit all
      </button>
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
