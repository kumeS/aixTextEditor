// "Draft a document by AI" dialog: a theme, an approximate target length, and
// optional reference material (pasted text, an attached .txt/.md/.rtf/.pdf
// file, or a fetched URL) that the draft should draw on. The draft itself is
// streamed into a new tab by `draftDocument`.

import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../api";
import { draftDocument } from "../fileActions";
import { useStore } from "../store";
import { CloseIcon, DraftIcon, ImportIcon, NetworkIcon, SpinnerIcon } from "./icons";

// Approximate target lengths (in words). `null` lets the model choose.
const LENGTHS: { label: string; value: number | null }[] = [
  { label: "自動 / Auto", value: null },
  { label: "短め / Short (~300)", value: 300 },
  { label: "標準 / Medium (~800)", value: 800 },
  { label: "長め / Long (~1500)", value: 1500 },
  { label: "とても長い / Very long (~3000)", value: 3000 },
];

export default function DraftModal() {
  const open_ = useStore((s) => s.draftOpen);
  const close = useStore((s) => s.closeDraft);
  const globalBusy = useStore((s) => s.globalBusy);
  const notify = useStore((s) => s.notify);

  const [theme, setTheme] = useState("");
  const [lengthIdx, setLengthIdx] = useState(0);
  const [reference, setReference] = useState("");
  const [url, setUrl] = useState("");
  const [working, setWorking] = useState<null | "file" | "url">(null);

  useEffect(() => {
    if (open_) {
      setTheme("");
      setLengthIdx(0);
      setReference("");
      setUrl("");
      setWorking(null);
    }
  }, [open_]);

  if (!open_) return null;

  const field =
    "w-full rounded-md border border-gray-300 px-3 py-2 text-sm outline-none focus:border-accent";

  const appendReference = (chunk: string, source: string) => {
    const piece = chunk.trim();
    if (!piece) {
      notify(`No readable text found in ${source}.`, "info");
      return;
    }
    setReference((r) =>
      r.trim()
        ? `${r.trim()}\n\n--- ${source} ---\n${piece}`
        : `--- ${source} ---\n${piece}`
    );
    notify(`Added reference from ${source}.`, "success");
  };

  const attachFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: "Reference", extensions: ["txt", "md", "markdown", "rtf", "pdf"] },
        ],
      });
      if (typeof selected !== "string") return;
      setWorking("file");
      const text = await api.readReferenceFile(selected);
      const name = selected.split(/[\\/]/).pop() ?? "file";
      appendReference(text, name);
    } catch (e) {
      notify(typeof e === "string" ? e : String(e), "error");
    } finally {
      setWorking(null);
    }
  };

  const fetchUrl = async () => {
    const u = url.trim();
    if (!u) return;
    try {
      setWorking("url");
      const text = await api.fetchUrlText(u);
      appendReference(text, u);
      setUrl("");
    } catch (e) {
      notify(typeof e === "string" ? e : String(e), "error");
    } finally {
      setWorking(null);
    }
  };

  const submit = () => {
    if (!theme.trim()) {
      notify("Enter a theme to draft about.", "info");
      return;
    }
    const words = LENGTHS[lengthIdx]?.value ?? undefined;
    close();
    void draftDocument(theme, words ?? undefined, reference.trim() || undefined);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      onMouseDown={close}
    >
      <div
        className="flex max-h-[90vh] w-full max-w-lg flex-col rounded-xl bg-white shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-gray-100 px-6 pb-3 pt-5">
          <h2 className="flex items-center gap-2 text-lg font-semibold text-ink">
            <DraftIcon /> Draft a document by AI
          </h2>
          <button onClick={close} className="text-ink-faint hover:text-ink" aria-label="Close">
            <CloseIcon />
          </button>
        </div>

        <div className="flex-1 space-y-4 overflow-y-auto px-6 py-4">
          <div>
            <label className="mb-1 block text-sm font-medium text-ink-soft">
              Theme / topic
            </label>
            <textarea
              value={theme}
              autoFocus
              onChange={(e) => setTheme(e.target.value)}
              placeholder="e.g. The role of attention mechanisms in NLP"
              rows={3}
              className={`${field} resize-y`}
            />
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-ink-soft">
              Approximate length
            </label>
            <select
              value={lengthIdx}
              onChange={(e) => setLengthIdx(Number(e.target.value))}
              className={field}
            >
              {LENGTHS.map((l, i) => (
                <option key={l.label} value={i}>
                  {l.label}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-ink-soft">
              Reference material <span className="text-ink-faint">(optional)</span>
            </label>
            <textarea
              value={reference}
              onChange={(e) => setReference(e.target.value)}
              placeholder="Paste notes or text here, and/or attach a file / fetch a URL below."
              rows={4}
              className={`${field} resize-y`}
            />
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={attachFile}
                disabled={working !== null}
                className="flex items-center gap-1.5 rounded-md border border-gray-300 px-2.5 py-1.5 text-sm text-ink-soft hover:bg-gray-100 disabled:opacity-50"
              >
                {working === "file" ? <SpinnerIcon /> : <ImportIcon className="h-4 w-4" />}
                Attach .txt / .md / .rtf / .pdf
              </button>
            </div>
            <div className="mt-2 flex gap-2">
              <input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void fetchUrl();
                  }
                }}
                placeholder="https://… reference URL"
                className={field}
              />
              <button
                type="button"
                onClick={() => void fetchUrl()}
                disabled={working !== null || !url.trim()}
                className="flex shrink-0 items-center gap-1.5 rounded-md border border-gray-300 px-3 py-2 text-sm text-ink-soft hover:bg-gray-100 disabled:opacity-50"
              >
                {working === "url" ? <SpinnerIcon /> : <NetworkIcon className="h-4 w-4" />}
                Fetch
              </button>
            </div>
            <p className="mt-1 text-xs text-ink-faint">
              The draft is grounded in this material (it won't copy it verbatim).
              PDF text extraction is best-effort.
            </p>
          </div>
        </div>

        <div className="flex shrink-0 justify-end gap-2 border-t border-gray-100 px-6 py-4">
          <button
            onClick={close}
            className="rounded-md px-4 py-2 text-sm text-ink-soft hover:bg-gray-100"
          >
            Cancel
          </button>
          <button
            onClick={submit}
            disabled={!!globalBusy || !theme.trim()}
            className="flex items-center gap-1.5 rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent-soft disabled:opacity-50"
          >
            <DraftIcon className="h-4 w-4" /> Draft
          </button>
        </div>
      </div>
    </div>
  );
}
