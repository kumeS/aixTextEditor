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
}

export interface Chunk {
  id: string;
  order: number;
  content: string;
  metadata: ChunkMetadata;
}

export interface Document {
  id: string;
  title: string;
  chunks: Chunk[];
  analysis?: AnalysisResult; // persisted relationship graph (spec §3.4)
}

export interface Settings {
  endpoint: string;
  model: string; // active text model id
  models: string[]; // selectable text-model list (persisted)
  imageModel: string; // active image-generation model id
  imageModels: string[]; // selectable image-model list (persisted)
  defaultTargetLanguage: string;
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
  | "custom";

export interface AiRequest {
  action: AiAction;
  text: string;
  contextBefore?: string;
  contextAfter?: string;
  targetLanguage?: string;
  style?: string; // target writing style for "proofread"
  instruction?: string;
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
