<div align="center">

<br/>

<img src="src-tauri/icons/128x128@2x.png" alt="aixTextEditor icon" width="160" />

<br/>

# ─── ✦ &nbsp; a i x T e x t E d i t o r &nbsp; ✦ ───

<h3>
  <em>🧠 AI &nbsp;×&nbsp; 📝 Chunks &nbsp;—&nbsp; The Next-Generation Writing Experience</em>
</h3>

<br/>

[![Version](https://img.shields.io/badge/🚀_v1.0.0-Release-0078D4?style=for-the-badge&logoColor=white)](https://github.com/kumeS/aixTextEditor/releases)
&nbsp;
[![License](https://img.shields.io/badge/📜_Artistic--2.0-License-2EA44F?style=for-the-badge)](LICENSE)
&nbsp;
[![Platform](https://img.shields.io/badge/🍎_macOS-Supported-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/kumeS/aixTextEditor/releases)

[![Tauri](https://img.shields.io/badge/⚡_Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=white)](https://v2.tauri.app)
&nbsp;
[![Rust](https://img.shields.io/badge/🦀_Rust-Backend-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
&nbsp;
[![React](https://img.shields.io/badge/⚛️_React-Frontend-61DAFB?style=for-the-badge&logo=react&logoColor=black)](https://react.dev)
&nbsp;
[![TypeScript](https://img.shields.io/badge/🔷_TypeScript-Typed-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org)

<br/>

---

<h2>📝 Every Paragraph is a Chunk.&nbsp; AI Understands Every Chunk.</h2>

---

</div>

<br/>

<div align="center">
<table>
<tr>
<td align="center" width="33%">
<h3>🚀 Streaming Draft</h3>
<p>Enter a theme → watch a fully structured<br/>first draft generate <strong>in real time</strong></p>
</td>
<td align="center" width="33%">
<h3>🎨 AI Image Generation</h3>
<p>Turn any paragraph into a visual —<br/><strong>images generated from your words</strong></p>
</td>
<td align="center" width="33%">
<h3>🔗 Relationship Graph</h3>
<p>Map the logical structure between<br/>paragraphs with <strong>interactive networks</strong></p>
</td>
</tr>
</table>
</div>

<br/>

> ### ✨ What is aixTextEditor?
>
> A **radically new** text editor where every paragraph lives as an independent **chunk** — think Jupyter Notebook cells, but for writing.
>
> Each chunk is a self-contained unit for editing **and** AI-powered operations:
> **translate** · **proofread** · **summarize** · **expand** · **generate diagrams** · **generate images** · **analyze relationships** — all executed with full awareness of surrounding context.
>
> _Supercharge your writing — papers, reports, technical docs — **with AI, one chunk at a time.**_

<br/>

## Features

| Area | What it does |
| --- | --- |
| **Multiple tabs** | Open and edit several documents at once. New tab via the tab-bar **＋** or `⌘/Ctrl+T`; each tab keeps its own document, file path, undo/redo history and analysis. |
| **Chunk editing** | Document = an ordered list of chunks. Split (`⌘/Ctrl+Shift+Enter`), merge (Backspace at start), reorder (↑/↓), add/delete, and move between chunks with the Up/Down arrows. |
| **Chunk types** | **Text**, **Heading** (`#`/`##`/`###`, levels 1–3), **Diagram** (Mermaid), and **Image**. Type `# `/`## `/`### ` at the start of a paragraph to turn it into a heading. |
| **Context-aware AI** | The nearest preceding/following text chunks are sent as context so generated text stays coherent. |
| **Per-chunk AI menu (✨)** | **Translate** (choose language), **Proofread** (choose a style: Academic / Formal / Concise / Plain / Persuasive / custom), **Expand**, **Add detail**, **Concentrate**, **Focus**, **Summarize**, **Generate diagram**, **Custom instruction**. `⌘/Ctrl+Enter` runs Proofread. |
| **Draft (streaming)** | Generate a full structured first draft from a theme; it **streams into a new tab in real time**, split into heading + paragraph chunks. |
| **Diagrams** | The model emits **Mermaid** code, rendered inline as SVG; diagram chunks keep an editable code area. |
| **Image generation** | Generate an image from a single paragraph (right-gutter button), or select multiple paragraphs (checkbox → floating **Generate image**) to combine them. Images are inserted as **image chunks** and can be reordered. Uses a separate image model (see Settings). |
| **Relationship graph** | **Analyze** builds a two-level network — **paragraph** nodes plus per-**sentence** nodes — with typed relations (cause, evidence, elaboration, contrast, …), drawn with **Cytoscape** (sentences nested under their paragraph). Click a node to jump to its paragraph. The graph is saved inside the `.aix` file. |
| **Import / Export** | One **Import / Export** menu (choose after clicking): import/export `.txt`, `.md`, `.rtf`. Native **`.aix`** format (Save/Open) preserves the full document — chunks, metadata and the analysis graph. |
| **Native menu** | The macOS/Windows menu bar (File / Edit / AI / Window) mirrors the in-app toolbar; its custom items drive the same actions. |
| **Security** | The API key is stored in the **OS keychain** (macOS Keychain / Windows Credential Manager / Linux Secret Service) — never written to disk in plaintext, never sent to the frontend. All network calls happen in Rust. |
| **Resilience** | Free models are rate-limited; API calls retry on HTTP 429 / transient 5xx with exponential backoff, and surface actionable errors. |
| **Performance** | Per-chunk selectors (only the edited paragraph re-renders), async ops with loading indicators, lazy-loaded Mermaid/Cytoscape. |

## Settings

Open with the gear icon or `⌘/Ctrl+,`:

- **OpenRouter API key** — stored in the OS keychain.
- **Endpoint URL** — any OpenAI-compatible chat-completions endpoint.
- **Model (text)** — a managed list you can select from / add to / remove. Used
  for writing, proofreading, drafting and analysis.
- **Model (image generation)** — a separate managed list for image models
  (e.g. Google "Nano Banana"). **Verify exact model ids on
  openrouter.ai/models** — image-model ids change frequently and the seeded ones
  are starting points.
- **Default translation language** — picked from a dropdown (English, 日本語,
  中文, 한국어, Español, Français, …).
- **Temperature**.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `↑` / `↓` (at a paragraph's top/bottom line) | Move to the previous / next chunk |
| `⌘/Ctrl + Enter` | Proofread the focused paragraph |
| `⌘/Ctrl + Shift + Enter` | Split paragraph at the caret |
| `Backspace` (at start) | Merge with previous paragraph (empty heading → text) |
| `⌘/Ctrl + T` | New tab |
| `⌘/Ctrl + S` / `O` | Save / Open `.aix` document |
| `⌘/Ctrl + Z` / `Shift+Z` | Undo / Redo |
| `⌘/Ctrl + ,` | Settings |

## Architecture

```
src/                     React frontend
  types.ts               TS mirror of the Rust model (camelCase)
  api.ts                 Typed invoke() wrappers (+ streaming Channel)
  store.ts               Zustand store: tabs, chunks, undo/redo, selection, UI
  aiActions.ts           AI orchestration (context, busy state, tab-race guards)
  fileActions.ts         New/Open/Import/Export/Draft (dialog plugin → Rust I/O)
  caret.ts               Visual-line caret detection (chunk Up/Down navigation)
  useShortcuts.ts        Global keyboard shortcuts
  components/            TabBar, Toolbar, Editor, ChunkView, ChunkAiMenu,
                         MermaidChunk, NetworkPanel, SettingsModal, PromptModal,
                         SelectionBar, Toasts, icons
src-tauri/src/           Rust backend
  models.rs              Document / Chunk / ChunkMetadata / Analysis* (serde)
  commands.rs            Tauri command surface
  ai.rs                  LlmProvider trait + OpenRouter impl (incl. SSE
                         streaming, image generation) + prompts
  fileio.rs              txt/md/rtf import-export, paragraph + heading chunking
  settings.rs            Settings JSON + OS keychain (keyring)
  menu.rs                Native application menu (emits events to the frontend)
  error.rs               Unified AppError
```

The AI layer is expressed as a trait (`LlmProvider`) so other OpenAI-compatible
providers can be slotted in without touching the command layer. State is managed
with **Zustand** for fine-grained per-chunk selector subscriptions; the active
tab lives in the top-level store fields while inactive tabs are kept as
snapshots, so existing chunk actions operate unchanged.

## Getting started

### Install (recommended) — Homebrew

```bash
brew tap kumeS/tap https://github.com/kumeS/aixTextEditor   # one-time: the formula lives in this repo
brew install kumeS/tap/aixtexteditor
```

> The `brew tap … <url>` line is required because the formula ships inside the app's
> own repo rather than a separate `homebrew-tap` repo. (If you later create a
> `kumeS/homebrew-tap` repo containing the formula, `brew install
> kumeS/tap/aixtexteditor` works on its own, with no `brew tap` step.)

This **builds aixTextEditor from source on your Mac**, so there is no notarization
/ *"app is damaged"* Gatekeeper prompt, and the binary matches your own CPU (Apple
Silicon or Intel). Homebrew installs Node and Rust automatically (Xcode Command
Line Tools required); the first build takes a few minutes.

Launch it from Spotlight as **aixTextEditor**, or:

```bash
aixtexteditor                                                    # CLI launcher
# …or add it to /Applications:
ln -sfn "$(brew --prefix)/opt/aixtexteditor/aixTextEditor.app" /Applications/
```

On first run, open **Settings** (gear icon, or `⌘/Ctrl+,`) and paste your
OpenRouter API key (free key at https://openrouter.ai/keys). The default text model
is free; change it (and the image model) to any id from
https://openrouter.ai/models. The key is stored in the macOS keychain, never on
disk in plaintext.

> **Prebuilt `.dmg` alternative.** A `.dmg` is also published on the
> [Releases](https://github.com/kumeS/aixTextEditor/releases) page (and via the
> Homebrew **cask** [`Casks/aix-text-editor.rb`](Casks/aix-text-editor.rb)). That
> build is *not notarized*, so macOS quarantines it on download — after installing,
> clear the flag once with
> `xattr -dr com.apple.quarantine "/Applications/aixTextEditor.app"`. The
> source build above avoids this entirely.

### Building from source manually (optional)

```bash
npm install
npm run tauri build    # → .app / .dmg under src-tauri/target/release/bundle/
```

## Notes & limitations

- AI features require an OpenRouter API key; image generation additionally
  requires an **image-capable** model id (verify on openrouter.ai/models).
- Free OpenRouter models share tight rate limits — if you hit 429 repeatedly,
  switch models in Settings, wait a minute, or add OpenRouter credit.
- `.rtf` conversion is pragmatic (text + paragraph structure), not full
  rich-text fidelity. `\'hh` byte escapes are decoded through Windows-1252, so
  smart quotes / dashes / bullets from Word-exported `.rtf` survive import.
- **The document round-trips losslessly only in the native `.aix` format.**
  Plain-text exports (`.txt`/`.md`/`.rtf`) are inherently flat: a blank line
  inside one paragraph chunk is indistinguishable from a chunk break on
  re-import, and images export as a placeholder. Markdown does round-trip
  headings and promotes a leading `# Heading` back to the document title.

## License

Copyright (c) 2026 Satoshi Kume. Released under the **Artistic License 2.0** —
see [LICENSE](LICENSE).
