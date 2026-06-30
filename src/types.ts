// TypeScript mirror of the Rust data model (src-tauri/src/models.rs, ai.rs,
// settings.rs). All Rust structs use `#[serde(rename_all = "camelCase")]`, so
// these field names line up 1:1 across the IPC boundary.

export type ChunkType = "text" | "diagram" | "heading" | "image";

export interface ChunkMetadata {
  chunkType: ChunkType;
  format?: string; // e.g. "mermaid" when chunkType === "diagram"
  level?: number; // 1–3 when chunkType === "heading"
  summary?: string;
  linkedChunks: string[];
  imagePrompt?: string; // image chunks: prompt used, for "regenerate"
  contentHistory?: string[]; // prior content values (text versions / image URLs)
  layout?: SlideLayout; // heading chunks: explicit slide-layout override
}

export interface Chunk {
  id: string;
  order: number;
  content: string;
  metadata: ChunkMetadata;
}

/**
 * Authoring mode of a document/tab. Chosen when the tab is created and FIXED for
 * its lifetime — an editor doc and a slide deck are independent things, never
 * converted into each other. Older .aix files without the field load as "editor".
 */
export type DocMode = "editor" | "slide";

export interface Document {
  id: string;
  title: string;
  chunks: Chunk[];
  mode?: DocMode; // defaults to "editor" when absent (back-compat)
  analysis?: AnalysisResult; // persisted relationship graph (spec §3.4)
}

// Slide deck model (v1.2.0). A slide reuses editor Chunks (heading = title,
// text = bullets, image, diagram) plus a layout; decks export to .pptx.
export type SlideLayout = "section" | "title-content" | "title-image";

export interface Slide {
  id: string;
  order: number;
  layout: SlideLayout;
  chunks: Chunk[];
  notes: string; // speaker notes (reserved; not yet emitted to PPTX)
}

export interface Deck {
  id: string;
  title: string;
  slides: Slide[];
}

/** Result of a PPTX export: slide count and any non-fatal notes. */
export interface PptxReport {
  slides: number;
  warnings: string[];
}

export interface Settings {
  endpoint: string;
  model: string; // active text model id
  models: string[]; // selectable text-model list (persisted)
  imageModel: string; // active image-generation model id
  imageModels: string[]; // selectable image-model list (persisted)
  defaultTargetLanguage: string; // "Default language" — global output language
  writingTone: string; // global writing tone applied to writing actions
  temperature: number;
}

export type AiAction =
  | "translate"
  | "proofread"
  | "summarize"
  | "expand"
  | "detailed"
  | "concentrate"
  | "focus"
  | "harmonize"
  | "custom";

export interface AiRequest {
  action: AiAction;
  text: string;
  contextBefore?: string;
  contextAfter?: string;
  targetLanguage?: string;
  style?: string; // target writing style for "proofread"
  instruction?: string;
  outputLanguage?: string; // pin output to the configured default language
  tone?: string; // global writing tone
  // T1 — whole-document context: the section heading the chunk lives under, a
  // compact document outline (headings + summaries), and graph-linked material.
  sectionHeading?: string;
  documentMap?: string;
  linkedContent?: string;
}

export type AnalysisNodeKind = "paragraph" | "sentence";

export interface AnalysisNode {
  id: string;
  label: string;
  summary: string;
  kind?: AnalysisNodeKind; // defaults to "paragraph"
  parent?: string; // owning paragraph id, for sentence nodes
}

export interface AnalysisEdge {
  source: string;
  target: string;
  relation: string;
}

export interface AnalysisResult {
  nodes: AnalysisNode[];
  edges: AnalysisEdge[];
}

export type ExportFormat = "txt" | "md" | "rtf";

/** Streaming draft events from the `ai_draft_stream` command channel. */
export type DraftEvent =
  | { kind: "update"; document: Document } // live snapshot (position-based ids)
  | { kind: "done"; document: Document }; // final document (real ids)
