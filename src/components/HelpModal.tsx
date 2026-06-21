// Help — a short guide to the recommended writing workflow, opened from the
// toolbar Help button and the native Help menu.

import { useStore } from "../store";
import {
  CloseIcon,
  DraftIcon,
  ImageIcon,
  NetworkIcon,
  SparklesIcon,
  SpeakerIcon,
  ExportIcon,
} from "./icons";

interface Step {
  icon: React.ReactNode;
  title: string;
  body: string;
}

const STEPS: Step[] = [
  {
    icon: <DraftIcon />,
    title: "1. Draft the whole document",
    body:
      "Click “Draft by AI”, enter a theme, pick an approximate length, and optionally attach reference text, a file (.txt/.md/.rtf/.pdf) or a URL. The AI streams a structured first draft — headings and paragraphs — into a new tab.",
  },
  {
    icon: <SparklesIcon />,
    title: "2. Refine paragraph by paragraph",
    body:
      "Focus a paragraph and use the ✨ menu in the left gutter: Translate, Proofread (with a style), Expand, Add detail, Concentrate, Focus, or a Custom instruction. Each action reads the surrounding paragraphs for context. ⌘/Ctrl+Enter runs a quick proofread; results are undoable with ⌘/Ctrl+Z.",
  },
  {
    icon: <ImageIcon />,
    title: "3. Add images & diagrams",
    body:
      "From a paragraph’s right gutter, generate an image, or convert it to a Mermaid diagram. Select several paragraphs (checkbox) and use the bottom bar to generate one combined image. Regenerate to get alternatives and pick your favourite.",
  },
  {
    icon: <NetworkIcon />,
    title: "4. Check the structure",
    body:
      "Use “Analyze” to extract the relationships between paragraphs and sentences as an interactive network graph. Click a node to jump to that paragraph.",
  },
  {
    icon: <SpeakerIcon />,
    title: "5. Listen back",
    body:
      "Use the read-aloud (🔊) control on a paragraph to hear it spoken — a quick way to catch awkward phrasing. The output language follows your Default language in Settings.",
  },
  {
    icon: <ExportIcon />,
    title: "6. Save & export",
    body:
      "Save as a native .aix file (lossless), or export to .txt / .md / .rtf / .pdf. PDF uses the system print dialog, so choose “Save as PDF”.",
  },
];

export default function HelpModal() {
  const open = useStore((s) => s.helpOpen);
  const close = useStore((s) => s.closeHelp);
  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      onMouseDown={close}
    >
      <div
        className="flex max-h-[90vh] w-full max-w-xl flex-col rounded-xl bg-white shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-gray-100 px-6 pb-3 pt-5">
          <h2 className="text-lg font-semibold text-ink">How to write with aixTextEditor</h2>
          <button onClick={close} className="text-ink-faint hover:text-ink" aria-label="Close">
            <CloseIcon />
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-5">
          <p className="text-sm text-ink-soft">
            A typical flow goes from a whole-document draft to fine-grained,
            paragraph-level editing, illustration, and export.
          </p>
          {STEPS.map((s) => (
            <div key={s.title} className="flex gap-3">
              <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent/10 text-accent">
                {s.icon}
              </div>
              <div>
                <div className="text-sm font-semibold text-ink">{s.title}</div>
                <p className="mt-0.5 text-sm leading-relaxed text-ink-soft">{s.body}</p>
              </div>
            </div>
          ))}
          <p className="border-t border-gray-100 pt-3 text-xs text-ink-faint">
            Tip: set your <strong>Default language</strong> and <strong>Writing tone</strong>{" "}
            in Settings — every AI action then keeps that language and voice.
          </p>
        </div>
      </div>
    </div>
  );
}
