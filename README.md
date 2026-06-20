# AIX Text Editor

An LLM-augmented, **chunk-based** text editor for academic papers and long-form
reports. Each paragraph is an independent "chunk" (like a Jupyter cell), so AI
assistance — translation, proofreading, diagram generation, relationship
analysis — always works with the surrounding context.

Built with **Tauri v2 (Rust)** + **React + TypeScript** + **Tailwind CSS**, using
**OpenRouter** (or any OpenAI-compatible endpoint) for AI.

## Features

| Area | What it does |
| --- | --- |
| **Chunk editing** | Document = ordered paragraph chunks. Split (`⌘/Ctrl+Shift+Enter`), merge (Backspace at start), reorder, add/delete, convert text⇄diagram. |
| **Context-aware AI** | The nearest preceding/following text chunks are sent as context so generated text stays coherent (spec §3.1). |
| **One-click actions** | Per-chunk ✨ menu: **Translate**, **Proofread**, **Summarize**, **Generate diagram**, **Custom instruction**. `⌘/Ctrl+Enter` runs Proofread. |
| **Diagrams** | The model emits **Mermaid** code, rendered inline as SVG. Diagram chunks have an editable code area. |
| **Network graph** | "Analyze" extracts logical relationships between paragraphs and draws an interactive **Cytoscape** graph; click a node to jump to its paragraph (spec §3.4). |
| **Import / Export** | `.txt`, `.md`, `.rtf` in and out. Native `.aix` format preserves full chunk JSON. |
| **Security** | API key is stored in the **OS keychain** (macOS Keychain / Windows Credential Manager / Linux Secret Service) — never on disk, never sent to the frontend. All network calls happen in Rust. |
| **Performance** | Per-chunk selectors (only the edited paragraph re-renders), async ops with loading indicators, and lazy-loaded Mermaid/Cytoscape. |

## Architecture

```
src/                     React frontend
  types.ts               TS mirror of the Rust model (camelCase)
  api.ts                 Typed invoke() wrappers
  store.ts               Zustand store (chunks, undo/redo, UI state)
  aiActions.ts           AI orchestration (context gathering, busy state)
  fileActions.ts         File menu (dialog plugin → Rust I/O)
  useShortcuts.ts        Global keyboard shortcuts
  components/            Toolbar, Editor, ChunkView, MermaidChunk,
                         NetworkPanel, SettingsModal, PromptModal, Toasts
src-tauri/src/           Rust backend
  models.rs              Document / Chunk / ChunkMetadata (serde)
  commands.rs            Tauri command surface
  ai.rs                  LlmProvider trait + OpenRouter impl + prompts
  fileio.rs              txt/md/rtf import-export, paragraph chunking
  settings.rs            Settings JSON + OS keychain (keyring)
  error.rs               Unified AppError
```

The AI layer is expressed as a trait (`LlmProvider`) so other providers (a local
Ollama bridge, another REST API) can be added without touching the command layer
(spec §4.3).

### State management (Zustand)

The development plan asked whether to use Redux or Zustand. **Zustand** was chosen:
it gives fine-grained selector subscriptions (so editing one paragraph re-renders
only that `ChunkView`), keeps undo/redo and IPC orchestration in one small store,
and adds almost no boilerplate — a good fit for the "Clarity & Simplicity" goal.

## Getting started

```bash
npm install
npm run tauri dev      # run the desktop app (hot reload)
```

Then open **Settings** (gear icon, or `⌘/Ctrl+,`) and paste your OpenRouter API
key. Get one free at https://openrouter.ai/keys. The default model is a free
model; change it to any current model id from https://openrouter.ai/models
(e.g. `anthropic/claude-3.5-sonnet`).

### Build installers

```bash
npm run tauri build    # → .dmg (macOS), .msi/.exe (Windows), etc.
```

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `⌘/Ctrl + Enter` | Run AI (proofread) on the focused paragraph |
| `⌘/Ctrl + Shift + Enter` | Split paragraph at the caret |
| `Backspace` (at start) | Merge with previous paragraph |
| `⌘/Ctrl + S` / `O` | Save / Open `.aix` document |
| `⌘/Ctrl + Z` / `Shift+Z` | Undo / Redo |
| `⌘/Ctrl + ,` | Settings |

## Notes

- Free OpenRouter model ids change over time; the default is a starting point and
  is fully overridable in Settings.
- `.rtf` conversion is pragmatic (text + paragraph structure), not full
  rich-text fidelity. `\'hh` byte escapes are decoded through Windows-1252, so
  smart quotes / dashes / bullets from Word-exported `.rtf` survive import.
- **Chunk boundaries and the relationship graph round-trip losslessly only in
  the native `.aix` format.** The plain-text exports (`.txt`/`.md`/`.rtf`) are
  inherently flat: a blank line inside one paragraph chunk is indistinguishable
  from a chunk break on re-import, so chunk segmentation (and the per-chunk
  `summary`/`linkedChunks` metadata) can shift across a text-format round-trip.
  Use `.aix` to preserve the document exactly. (Markdown does round-trip the
  title: a leading `# Heading` is promoted back to the document title.)
