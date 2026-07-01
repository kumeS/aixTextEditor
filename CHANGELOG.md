# Changelog

## v1.2.0 — 2026-07-01

Slides. Adds a full presentation mode alongside the text editor, plus a
security- and stability-focused bug-fix pass.

### Slides & PPTX

- **Slides mode** — a new **Editor / Slides** toolbar toggle turns the current
  document into an editable deck (headings → titles, paragraphs → bullets), with
  a live deck preview kept in sync with the export. New tabs are added directly
  with the "+" button.
- **Per-slide layouts** — apply `section` / `title-content` / `title-image` to
  any slide (including a heading-less opening slide), so a slide isn't forced
  into a bullet list.
- **Title & subtitle** — mark a paragraph as a **subtitle** (Add subtitle, or the
  "S" control); it renders under the title and fills the slide's subtitle box —
  the same structure AI Draft produces, now insertable by hand.
- **Detach a slide** — "Summarize → slide" turns a slide's text into its own
  concise summary that lives independently of the document prose (edit it in
  place, or **Re-link** to reconnect); apply a custom layout on top.
- **Export to PowerPoint (`.pptx`)** — deterministic, AI-free conversion, from
  both the toolbar and the native File ▸ Export menu; on-screen preview and the
  exported deck now match (section layout, subtitles, heading-less titles).

### Fixes & hardening (Bug_report_v1 pass)

- Fixed a PPTX export failure caused by control characters in slide text
  (previously produced an unopenable file); broader image-format support
  (PNG/JPEG/GIF/BMP) with clear warnings for unsupported ones.
- `.aix` files are now validated and repaired on open (duplicate ids, dangling
  references, out-of-range values) instead of silently corrupting state.
- Network hardening for document image/URL fetches: SSRF guard against
  private/loopback/metadata addresses, response **size limits** + timeouts, and
  a tightened **Content-Security-Policy**; path-extension checks on writes.
- Per-tab in-flight state is fully isolated — no spinner/analysis leaks between
  open tabs.
- **Autosave & crash recovery** of open tabs; the relationship graph now flags
  itself out-of-date when its source paragraphs change (and prunes deleted ones).
- Read-aloud (text-to-speech) lifecycle fixed — the button clears when playback
  ends and no longer cross-wires between paragraphs.
- **Save As…**, native-menu PPTX export, undo-consistent analysis, and slide
  editing that respects slide boundaries.

### Other

- On macOS, **closing the window keeps the app running** — re-open it from the
  Dock; Cmd+Q quits.
- Pinned build-time **esbuild** (>= 0.28.1) to patch GHSA-g7r4-m6w7-qqqr.

## v1.1.0 — 2026-06-22

Feature update focused on language consistency, drafting, illustration and
accessibility.

### Settings
- **Default language** moved to the top of Settings and renamed from “Default
  translation language”. It is now the output language for **every** AI action,
  so results no longer drift (e.g. proofreading Japanese keeps it Japanese).
- **Writing tone** — choose a global voice (Blog / Memo / Report / Scientific /
  Academic paper) applied to all writing actions.
- Expanded pre-registered **text models** (default: `deepseek/deepseek-v4-flash`)
  and **image models** (Grok Imagine, Recraft v4 Pro, GPT-5.4-image, FLUX.2).
- Endpoint help now recommends the OpenRouter default and documents using a
  local **Ollama** endpoint (API key optional for local endpoints).

### Drafting
- **Draft a document by AI** (renamed) with an approximate **length** setting and
  attachable **reference material** — pasted text, a file (`.txt/.md/.rtf/.pdf`),
  or a fetched **URL** the draft is grounded in.

### Per-paragraph AI
- **Streaming** output for per-chunk actions (translate, proofread, …), like Draft.
- **Revise with context** — rewrite a paragraph to fit its neighbours.
- **Version history per paragraph** — every AI edit saves the previous version;
  swap back at any time.
- **Change highlight** — after proofreading, see exactly what changed (word-level).
- **Multi-paragraph editing** — apply one instruction to all selected paragraphs.
- AI actions (proofread / translate / custom) now available on **headings**, in
  addition to the H1/H2/H3 picker.

### Images & figures
- **Regenerate** button and **version gallery** on image chunks — keep every
  alternative and pick the final one.
- **Presentation figure** generation — a clean diagram-style illustration,
  separate from literal image generation.

### Other
- **Read aloud** (text-to-speech) for any paragraph (macOS speech synthesizer).
- **PDF export** via the system print dialog (handles CJK fonts correctly).
- **Help** menu — an in-app guide to the writing workflow (toolbar + native menu).
- **Tooltips** on the gutter and menu controls.

## v1.0.0 — 2026-06-21

First public release.

### Editing
- Chunk-based editor (text / heading / diagram / image chunks).
- **Multiple tabs** — manage several documents at once; each tab keeps its own
  document, file, undo/redo history and relationship graph (`⌘/Ctrl+T`, tab-bar ＋).
- **Headings** — `#`/`##`/`###` become heading chunks (levels 1–3); type the
  marker at a paragraph's start to convert.
- Split / merge / reorder chunks; move between chunks with the Up/Down arrows.

### AI (OpenRouter)
- Per-chunk ✨ menu: Translate, Proofread (selectable style), Expand, Add detail,
  Concentrate, Focus, Summarize, Generate diagram, Custom instruction.
- **Draft** — generate a structured document from a theme, streamed into a new
  tab in real time.
- **Image generation** — per paragraph or from a multi-paragraph selection;
  inserted as movable image chunks. Uses a separately-configured image model.
- **Relationship graph** — paragraph + per-sentence nodes with typed relations,
  rendered with Cytoscape; persisted in the `.aix` file.
- Context-aware prompts (neighbouring paragraphs); automatic retry/back-off on
  rate limits (HTTP 429).

### Files & platform
- Native `.aix` save/open (lossless); merged **Import / Export** menu for
  `.txt` / `.md` / `.rtf`.
- Native application menu mirrors the in-app toolbar.
- API key stored in the OS keychain; all network calls happen in Rust.

### Project
- Licensed under the **Artistic License 2.0** (© 2026 Satoshi Kume).
- Homebrew cask template under `Casks/`.
