//! Core document data model (chunk-based architecture).
//!
//! A `Document` is an ordered collection of paragraph-sized `Chunk`s — the
//! editor treats each paragraph like a Jupyter cell. The model mirrors the
//! conceptual structure in the requirements spec (§5): every chunk carries
//! `metadata` with its type, optional diagram `format`, an optional `summary`
//! used for relationship analysis, and `linkedChunks` for the network graph.
//!
//! All structs serialize as camelCase to match the TypeScript types 1:1.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CHUNK_TYPE_TEXT: &str = "text";
pub const CHUNK_TYPE_DIAGRAM: &str = "diagram";
pub const CHUNK_TYPE_HEADING: &str = "heading";
pub const CHUNK_TYPE_IMAGE: &str = "image";
pub const DIAGRAM_FORMAT_MERMAID: &str = "mermaid";

/// Generate a fresh UUID v4 string id.
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMetadata {
    /// "text", "diagram", or "heading".
    #[serde(default = "default_chunk_type")]
    pub chunk_type: String,
    /// Diagram notation when `chunk_type == "diagram"` (e.g. "mermaid").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Heading level (1–3) when `chunk_type == "heading"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// One-line summary used by the relationship/network analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Ids of chunks this chunk is logically linked to.
    #[serde(default)]
    pub linked_chunks: Vec<String>,
}

fn default_chunk_type() -> String {
    CHUNK_TYPE_TEXT.to_string()
}

impl Default for ChunkMetadata {
    fn default() -> Self {
        Self {
            chunk_type: default_chunk_type(),
            format: None,
            level: None,
            summary: None,
            linked_chunks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub order: u32,
    pub content: String,
    #[serde(default)]
    pub metadata: ChunkMetadata,
}

impl Chunk {
    pub fn new_text(order: u32, content: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            order,
            content: content.into(),
            metadata: ChunkMetadata::default(),
        }
    }

    pub fn new_diagram(order: u32, code: impl Into<String>, format: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            order,
            content: code.into(),
            metadata: ChunkMetadata {
                chunk_type: CHUNK_TYPE_DIAGRAM.to_string(),
                format: Some(format.into()),
                level: None,
                summary: None,
                linked_chunks: Vec::new(),
            },
        }
    }

    /// A heading chunk (`# Title`). `level` is clamped to 1–3.
    pub fn new_heading(order: u32, level: u8, text: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            order,
            content: text.into(),
            metadata: ChunkMetadata {
                chunk_type: CHUNK_TYPE_HEADING.to_string(),
                format: None,
                level: Some(level.clamp(1, 3)),
                summary: None,
                linked_chunks: Vec::new(),
            },
        }
    }

    pub fn is_diagram(&self) -> bool {
        self.metadata.chunk_type == CHUNK_TYPE_DIAGRAM
    }

    pub fn is_heading(&self) -> bool {
        self.metadata.chunk_type == CHUNK_TYPE_HEADING
    }

    pub fn is_image(&self) -> bool {
        self.metadata.chunk_type == CHUNK_TYPE_IMAGE
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub chunks: Vec<Chunk>,
    /// Persisted relationship graph (spec §3.4) so it survives save/reopen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisResult>,
}

impl Document {
    pub fn new(title: &str) -> Self {
        Self {
            id: new_id(),
            title: title.to_string(),
            chunks: Vec::new(),
            analysis: None,
        }
    }
}

// ----- relationship analysis graph (spec §3.4) -----------------------------
//
// The graph has two node kinds: "paragraph" (id = chunk id) and "sentence"
// (id = "<paragraphId>#s<n>", `parent` = the owning paragraph). Edges carry a
// `relation` property naming the relationship type (cause, evidence, …).

fn default_node_kind() -> String {
    "paragraph".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub summary: String,
    /// "paragraph" or "sentence".
    #[serde(default = "default_node_kind")]
    pub kind: String,
    /// For sentence nodes: id of the owning paragraph (chunk).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisEdge {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    #[serde(default)]
    pub nodes: Vec<AnalysisNode>,
    #[serde(default)]
    pub edges: Vec<AnalysisEdge>,
}
