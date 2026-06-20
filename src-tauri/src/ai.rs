//! AI integration.
//!
//! Per requirement §4.3 the network/LLM layer is expressed as a trait
//! (`LlmProvider`) so that other providers (a local Ollama endpoint, a different
//! REST API, ...) can be slotted in without touching the command layer. The
//! current concrete implementation talks to an OpenAI-compatible chat-completions
//! endpoint (OpenRouter by default).
//!
//! Requests carry the surrounding paragraph chunks as context (requirement
//! §3.1, "context-aware editing") so the model produces logically coherent text.

use crate::error::{AppError, AppResult};
use crate::models::{Document, CHUNK_TYPE_TEXT};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Configuration needed to reach a provider for a single request.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub temperature: f32,
}

/// The extensibility seam: any chat-style LLM backend implements this.
#[allow(async_fn_in_trait)]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> AppResult<String>;
}

/// OpenAI-compatible chat completions provider (OpenRouter by default).
pub struct OpenRouterProvider {
    pub config: LlmConfig,
}

impl OpenRouterProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }
}

impl LlmProvider for OpenRouterProvider {
    async fn complete(&self, system: &str, user: &str) -> AppResult<String> {
        let client = reqwest::Client::new();
        let payload = json!({
            "model": self.config.model,
            "temperature": self.config.temperature,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ]
        });

        // Free OpenRouter models share tight rate limits and frequently return
        // 429 (or transient 5xx) under load. Retry a few times with backoff,
        // honouring Retry-After, before surfacing an actionable error.
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt: u32 = 0;
        let body: serde_json::Value = loop {
            attempt += 1;
            let res = client
                .post(&self.config.endpoint)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                // OpenRouter attribution headers (optional but recommended).
                .header("HTTP-Referer", "https://github.com/kumeS/AIX_Text_Editor")
                .header("X-Title", "AIX Text Editor")
                .json(&payload)
                .send()
                .await?;

            let status = res.status();

            // 429 (rate limit) and 5xx are transient — retry with backoff.
            if (status.as_u16() == 429 || status.is_server_error()) && attempt < MAX_ATTEMPTS {
                let retry_after = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok());
                // Honour Retry-After, else exponential backoff; cap so the UI
                // never hangs for long.
                let wait = retry_after.unwrap_or(1u64 << (attempt - 1)).min(5);
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }

            let body: serde_json::Value = res.json().await?;
            if status.is_success() {
                break body;
            }

            let provider_msg = body["error"]["message"]
                .as_str()
                .or_else(|| body["error"].as_str())
                .unwrap_or("unknown error");
            let msg = match status.as_u16() {
                429 => format!(
                    "Rate limited (429). Free OpenRouter models share tight limits — wait a minute and \
                     retry, switch to another model in Settings, or add credit at openrouter.ai. \
                     (provider: {provider_msg})"
                ),
                401 | 403 => format!(
                    "Authorization failed ({}). Check your OpenRouter API key in Settings. \
                     (provider: {provider_msg})",
                    status.as_u16()
                ),
                404 => format!(
                    "Model not found (404). Verify the model id in Settings — it may be unavailable or \
                     have changed. (provider: {provider_msg})"
                ),
                code => format!("API {code}: {provider_msg}"),
            };
            return Err(AppError::Network(msg));
        };

        let out = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        if out.is_empty() {
            return Err(AppError::Network(
                "The model returned an empty response.".to_string(),
            ));
        }
        Ok(out)
    }
}

// ----- request / result types --------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequest {
    /// "translate" | "proofread" | "summarize" | "custom"
    pub action: String,
    pub text: String,
    #[serde(default)]
    pub context_before: Option<String>,
    #[serde(default)]
    pub context_after: Option<String>,
    #[serde(default)]
    pub target_language: Option<String>,
    /// Target writing style for the "proofread" action (e.g. "concise and
    /// formal"). When empty, proofreading defaults to a scholarly tone.
    #[serde(default)]
    pub style: Option<String>,
    /// Free-form instruction for the "custom" action.
    #[serde(default)]
    pub instruction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub summary: String,
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
    // Models sometimes omit an empty array entirely (e.g. nodes-only when no
    // relations are found). Default both so a partial result still parses
    // instead of failing the whole analysis with a "missing field" error.
    #[serde(default)]
    pub nodes: Vec<AnalysisNode>,
    #[serde(default)]
    pub edges: Vec<AnalysisEdge>,
}

// ----- high-level operations ----------------------------------------------

fn context_block(req: &AiRequest) -> String {
    let mut ctx = String::new();
    if let Some(before) = req.context_before.as_ref().filter(|s| !s.trim().is_empty()) {
        ctx.push_str("[Preceding paragraph]\n");
        ctx.push_str(before.trim());
        ctx.push_str("\n\n");
    }
    if let Some(after) = req.context_after.as_ref().filter(|s| !s.trim().is_empty()) {
        ctx.push_str("[Following paragraph]\n");
        ctx.push_str(after.trim());
        ctx.push_str("\n\n");
    }
    ctx
}

/// Run a one-click text action (translate / proofread / summarize / custom).
pub async fn run_action(config: &LlmConfig, req: &AiRequest) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());

    let system = match req.action.as_str() {
        "translate" => {
            let lang = req
                .target_language
                .clone()
                .unwrap_or_else(|| "English".to_string());
            format!(
                "You are an expert academic translator. Translate the user's target paragraph into {lang}, \
                 preserving meaning, terminology and an academic tone. Use the surrounding context only to \
                 disambiguate; do not translate or repeat the context. Output ONLY the translated paragraph, \
                 with no preamble, notes, or quotation marks."
            )
        }
        "proofread" => {
            let style = req
                .style
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("scholarly and academic");
            format!(
                "You are a meticulous copy-editor. Correct spelling, grammar and punctuation, improve \
                 clarity and concision, and adjust the writing toward a {style} style, while preserving \
                 the author's meaning and language. Use the surrounding context only for consistency. \
                 Output ONLY the revised paragraph, with no preamble, explanations, or quotation marks."
            )
        }
        "summarize" => "You are an expert academic editor. Write a single concise sentence summarizing the \
             target paragraph, suitable as metadata. Output ONLY that sentence."
            .to_string(),
        "expand" => "You are an academic writing assistant. Expand and develop the target paragraph: add \
             supporting sentences, elaboration, and smooth transitions so it reads more thoroughly, while \
             preserving the original meaning, language, and tone. Do not introduce unrelated claims or \
             fabricated facts. Use the surrounding context only for coherence; do not repeat it. Output \
             ONLY the expanded paragraph, with no preamble, explanation, or quotation marks."
            .to_string(),
        "detailed" => "You are an academic writing assistant. Rewrite the target paragraph in greater \
             detail: turn general statements into specific, concrete ones and add clarifying explanation, \
             while preserving the original meaning, language, and tone. Do not invent false facts, data, \
             or citations. Use the surrounding context only for coherence; do not repeat it. Output ONLY \
             the revised paragraph, with no preamble, explanation, or quotation marks."
            .to_string(),
        "concentrate" => "You are an academic writing assistant. Condense the target paragraph: remove \
             redundancy and wordiness and tighten the phrasing so it is more concise, while keeping all \
             key information and preserving the original meaning, language, and tone. Use the surrounding \
             context only for coherence; do not repeat it. Output ONLY the condensed paragraph, with no \
             preamble, explanation, or quotation marks."
            .to_string(),
        "focus" => "You are an academic writing assistant. Sharpen the target paragraph so it centers \
             clearly on its main point: cut tangential or digressive material and keep the core argument, \
             while preserving the original meaning, language, and tone. Use the surrounding context only \
             for coherence; do not repeat it. Output ONLY the focused paragraph, with no preamble, \
             explanation, or quotation marks."
            .to_string(),
        "custom" => req
            .instruction
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "You are a helpful academic writing assistant. Improve the target paragraph.".to_string()),
        other => return Err(AppError::Other(format!("Unknown AI action: '{other}'"))),
    };

    let user = format!(
        "{}[Target paragraph]\n{}",
        context_block(req),
        req.text.trim()
    );

    provider.complete(&system, &user).await
}

fn strip_code_fences(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        // Drop the optional language tag on the first line and the trailing fence.
        let after_lang = rest.splitn(2, '\n').nth(1).unwrap_or("");
        let body = after_lang
            .trim_end()
            .strip_suffix("```")
            .unwrap_or(after_lang);
        return body.trim().to_string();
    }
    t.to_string()
}

/// Generate Mermaid diagram code from a description / paragraph.
pub async fn generate_diagram(
    config: &LlmConfig,
    text: &str,
    instruction: Option<&str>,
) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());
    let extra = instruction
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!(" Additional instruction: {s}."))
        .unwrap_or_default();
    let system = format!(
        "You are a diagramming assistant. Convert the user's text into a single valid Mermaid.js diagram \
         (flowchart, sequence, class, or mind map — choose what best fits the content). Output ONLY raw \
         Mermaid code. Do NOT wrap it in Markdown code fences and do NOT add any explanation.{extra}"
    );
    let raw = provider.complete(&system, text.trim()).await?;
    Ok(strip_code_fences(&raw))
}

/// Generate a structured first draft (Markdown headings + paragraphs) on a theme.
pub async fn generate_draft(config: &LlmConfig, theme: &str) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());
    let system = "You are an academic writing assistant. Write a coherent, well-structured first draft on \
         the user's theme. Organise it with Markdown ATX headings to show the document structure — '#' for \
         chapter-level headings, '##' for sections, '###' for subsections — and write the body as clear \
         prose paragraphs separated by blank lines. Order the material logically (e.g. introduction, \
         development, conclusion). Do NOT restate the theme verbatim as the very first line, and do NOT use \
         bullet lists, tables, code fences, or any commentary — output ONLY the draft itself (headings and \
         paragraphs).";
    provider.complete(system, theme.trim()).await
}

fn extract_json(s: &str) -> &str {
    let start = s.find('{');
    let end = s.rfind('}');
    match (start, end) {
        (Some(a), Some(b)) if b > a => &s[a..=b],
        _ => s,
    }
}

/// Analyze the whole document and extract a relationship graph between chunks.
pub async fn analyze_document(config: &LlmConfig, doc: &Document) -> AppResult<AnalysisResult> {
    let provider = OpenRouterProvider::new(config.clone());

    // Only feed prose paragraphs to the analyzer; diagrams and headings carry no
    // paragraph-level relations (and headings aren't restored as graph nodes on
    // reload, so including them here would make the live/reopened graph diverge).
    let mut listing = String::new();
    for chunk in doc.chunks.iter() {
        if chunk.metadata.chunk_type != CHUNK_TYPE_TEXT {
            continue;
        }
        let snippet: String = chunk.content.chars().take(600).collect();
        if snippet.trim().is_empty() {
            continue;
        }
        listing.push_str(&format!("- id: {}\n  text: {}\n", chunk.id, snippet.replace('\n', " ")));
    }

    if listing.trim().is_empty() {
        return Ok(AnalysisResult { nodes: vec![], edges: vec![] });
    }

    let system = "You are a discourse-analysis engine for academic writing. Given a list of paragraphs \
         (each with an id), identify the logical relationships between them (e.g. cause/effect, \
         claim/evidence, premise/conclusion, elaboration, contrast). Respond with STRICT JSON only, no \
         markdown, of the exact shape: \
         {\"nodes\":[{\"id\":\"<paragraph id>\",\"label\":\"<3-6 word topic>\",\"summary\":\"<one sentence>\"}],\
         \"edges\":[{\"source\":\"<id>\",\"target\":\"<id>\",\"relation\":\"<short relation label>\"}]}. \
         Use ONLY the provided ids. Include every paragraph as a node.";

    let user = format!("Paragraphs:\n{listing}");
    let raw = provider.complete(system, &user).await?;
    let json_str = extract_json(&raw);
    let result: AnalysisResult = serde_json::from_str(json_str).map_err(|e| {
        AppError::Other(format!("Could not parse analysis JSON from model: {e}"))
    })?;
    Ok(result)
}
