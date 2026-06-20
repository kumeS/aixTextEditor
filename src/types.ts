// TypeScript mirror of the Rust data model (src-tauri/src/models.rs, ai.rs,
// settings.rs). All Rust structs use `#[serde(rename_all = "camelCase")]`, so
// these field names line up 1:1 across the IPC boundary.

export type ChunkType = "text" | "diagram";

export interface ChunkMetadata {
  chunkType: ChunkType;
  format?: string; // e.g. "mermaid" when chunkType === "diagram"
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
}

export interface Settings {
  endpoint: string;
  model: string;
  defaultTargetLanguage: string;
  temperature: number;
}

export type AiAction = "translate" | "proofread" | "summarize" | "custom";

export interface AiRequest {
  action: AiAction;
  text: string;
  contextBefore?: string;
  contextAfter?: string;
  targetLanguage?: string;
  instruction?: string;
}

export interface AnalysisNode {
  id: string;
  label: string;
  summary: string;
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
