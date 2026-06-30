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
    /// For image chunks: the prompt/source text used to generate the image, so a
    /// "regenerate" can re-run the same request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_prompt: Option<String>,
    /// Prior content values for this chunk (text: previous paragraph versions;
    /// image: previously generated image URLs) so the user can swap back to an
    /// earlier version. The current value lives in `Chunk::content`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_history: Vec<String>,
    /// For a HEADING chunk that begins a slide: an explicit slide-layout override
    /// ("section" | "title-content" | "title-image"). When absent, the layout is
    /// auto-picked from the slide's content (see `deck.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
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
            image_prompt: None,
            content_history: Vec::new(),
            layout: None,
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
                image_prompt: None,
                content_history: Vec::new(),
                layout: None,
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
                image_prompt: None,
                content_history: Vec::new(),
                layout: None,
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

/// Authoring mode, fixed at creation: "editor" (prose document) or "slide"
/// (a deck). Persisted so a saved deck reopens as a deck.
pub const DOC_MODE_EDITOR: &str = "editor";
/// The "slide" mode literal — set on the frontend; kept here as the documented
/// contract for the field's valid values.
#[allow(dead_code)]
pub const DOC_MODE_SLIDE: &str = "slide";

fn default_doc_mode() -> String {
    DOC_MODE_EDITOR.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub chunks: Vec<Chunk>,
    /// "editor" | "slide". Older .aix files without it load as "editor".
    #[serde(default = "default_doc_mode")]
    pub mode: String,
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
            mode: default_doc_mode(),
            analysis: None,
        }
    }
}

// ----- slide deck model (v1.2.0) -------------------------------------------
//
// A `Deck` is an ordered list of `Slide`s; a slide is a `layout` plus a small
// group of the same `Chunk`s used by the text editor (heading = title,
// text = bullets, image, diagram), so every per-chunk AI action works inside
// slides unchanged. Decks are export-only to `.pptx` for now (see `pptx.rs`);
// `deck::document_to_deck` derives one from a text document.

pub const SLIDE_LAYOUT_SECTION: &str = "section";
pub const SLIDE_LAYOUT_TITLE_CONTENT: &str = "title-content";
pub const SLIDE_LAYOUT_TITLE_IMAGE: &str = "title-image";

fn default_layout() -> String {
    SLIDE_LAYOUT_TITLE_CONTENT.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    pub order: u32,
    /// "section" | "title-content" | "title-image".
    #[serde(default = "default_layout")]
    pub layout: String,
    /// Reused editor chunks: heading = title, text = bullets, image, diagram.
    #[serde(default)]
    pub chunks: Vec<Chunk>,
    /// Speaker notes (not yet emitted to PPTX; reserved for a later phase).
    #[serde(default)]
    pub notes: String,
}

impl Slide {
    pub fn new(order: u32, layout: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            order,
            layout: layout.into(),
            chunks: Vec::new(),
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deck {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub slides: Vec<Slide>,
}

impl Deck {
    pub fn new(title: &str) -> Self {
        Self {
            id: new_id(),
            title: title.to_string(),
            slides: Vec::new(),
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
