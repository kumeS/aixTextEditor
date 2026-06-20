// The per-chunk "✨" AI menu shown in the left gutter (spec §3.2 one-click
// actions). Opens a small popover with context-aware actions.

import { useEffect, useRef, useState } from "react";
import {
  runChunkAction,
  generateDiagramFromChunk,
} from "../aiActions";
import { useStore } from "../store";
import { promptDialog } from "./PromptModal";
import {
  FlowIcon,
  LanguagesIcon,
  SparklesIcon,
  SpinnerIcon,
  SummaryIcon,
  WandIcon,
} from "./icons";

interface Props {
  chunkId: string;
  isText: boolean;
  busy: boolean;
}

export default function ChunkAiMenu({ chunkId, isText, busy }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const defaultLang = useStore((s) => s.settings?.defaultTargetLanguage ?? "English");

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const close = () => setOpen(false);

  const onTranslate = async () => {
    close();
    const lang = await promptDialog({
      title: "Translate paragraph",
      label: "Target language",
      defaultValue: defaultLang,
      placeholder: "e.g. English, 日本語, Français",
      submitLabel: "Translate",
    });
    if (lang === null) return;
    await runChunkAction(chunkId, "translate", { targetLanguage: lang });
  };

  const onProofread = async () => {
    close();
    await runChunkAction(chunkId, "proofread");
  };

  const onSummarize = async () => {
    close();
    await runChunkAction(chunkId, "summarize");
  };

  const onDiagram = async () => {
    close();
    const instruction = await promptDialog({
      title: "Generate diagram",
      label: "Optional guidance for the diagram (leave blank for automatic)",
      defaultValue: "",
      placeholder: "e.g. as a flowchart of the process",
      submitLabel: "Generate",
    });
    if (instruction === null) return;
    await generateDiagramFromChunk(chunkId, instruction || undefined);
  };

  const onCustom = async () => {
    close();
    const instruction = await promptDialog({
      title: "Custom AI instruction",
      label: "Describe what the AI should do with this paragraph",
      placeholder: "e.g. Rewrite this for a general audience",
      multiline: true,
      submitLabel: "Run",
    });
    if (!instruction) return;
    await runChunkAction(chunkId, "custom", { instruction });
  };

  const item =
    "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm text-ink-soft hover:bg-accent/10 hover:text-accent";

  return (
    <div ref={rootRef} className="relative">
      <button
        title="AI actions"
        aria-label="AI actions"
        onClick={() => setOpen((v) => !v)}
        className={`flex h-7 w-7 items-center justify-center rounded-full text-ink-faint transition-colors hover:bg-accent/10 hover:text-accent ${
          open ? "bg-accent/10 text-accent" : ""
        }`}
      >
        {busy ? <SpinnerIcon className="text-accent" /> : <SparklesIcon />}
      </button>

      {open && (
        <div className="absolute left-0 top-8 z-30 w-52 rounded-lg border border-gray-200 bg-white p-1 shadow-lg">
          {isText && (
            <>
              <button className={item} onClick={onTranslate}>
                <LanguagesIcon /> Translate…
              </button>
              <button className={item} onClick={onProofread}>
                <WandIcon /> Proofread
              </button>
              <button className={item} onClick={onSummarize}>
                <SummaryIcon /> Summarize
              </button>
              <button className={item} onClick={onDiagram}>
                <FlowIcon /> Generate diagram…
              </button>
              <div className="my-1 border-t border-gray-100" />
              <button className={item} onClick={onCustom}>
                <SparklesIcon /> Custom instruction…
              </button>
            </>
          )}
          {!isText && (
            <div className="px-2.5 py-1.5 text-sm text-ink-faint">
              Edit the Mermaid code below to update this diagram.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
