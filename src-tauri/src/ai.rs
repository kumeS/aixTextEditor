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
use crate::models::{AnalysisResult, Document, CHUNK_TYPE_TEXT};
use futures_util::StreamExt;
use serde::Deserialize;
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
                .header("X-Title", "aixTextEditor")
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

impl OpenRouterProvider {
    /// Stream a completion token-by-token. `on_delta` is called with the FULL
    /// accumulated text each time new content arrives; the final text is returned.
    pub async fn complete_stream<F: FnMut(&str)>(
        &self,
        system: &str,
        user: &str,
        mut on_delta: F,
    ) -> AppResult<String> {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.config.endpoint)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("HTTP-Referer", "https://github.com/kumeS/AIX_Text_Editor")
            .header("X-Title", "aixTextEditor")
            .json(&json!({
                "model": self.config.model,
                "temperature": self.config.temperature,
                "stream": true,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": user }
                ]
            }))
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body: serde_json::Value = res.json().await.unwrap_or_else(|_| json!({}));
            let provider_msg = body["error"]["message"]
                .as_str()
                .or_else(|| body["error"].as_str())
                .unwrap_or("unknown error");
            return Err(AppError::Network(match status.as_u16() {
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
                code => format!("API {code}: {provider_msg}"),
            }));
        }

        // Server-Sent Events: bytes arrive on arbitrary boundaries, so buffer
        // raw bytes and only parse COMPLETE lines (split on '\n'). A multibyte
        // UTF-8 char never contains 0x0A, so splitting on the newline byte is safe.
        let mut stream = res.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut full = String::new();

        while let Some(item) = stream.next().await {
            let bytes = item.map_err(AppError::from)?;
            buf.extend_from_slice(&bytes);

            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len() - 1]);
                let line = line.trim_end_matches('\r').trim();

                if line.is_empty() || line.starts_with(':') {
                    continue; // blank line or SSE comment / keep-alive
                }
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(full);
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                        if !delta.is_empty() {
                            full.push_str(delta);
                            on_delta(&full);
                        }
                    }
                }
            }
        }
        Ok(full)
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
    /// formal"). When empty, proofreading defaults to the global writing tone,
    /// then to a scholarly tone.
    #[serde(default)]
    pub style: Option<String>,
    /// Free-form instruction for the "custom" action.
    #[serde(default)]
    pub instruction: Option<String>,
    /// The configured default language. Every non-translate action is pinned to
    /// write its output in this language so results never silently drift away
    /// from what the user set in Settings.
    #[serde(default)]
    pub output_language: Option<String>,
    /// The global writing tone (blog / memo / report / scientific / academic).
    /// Applied to the open-ended writing actions for a consistent voice.
    #[serde(default)]
    pub tone: Option<String>,
}

/// Trailing constraints appended to a writing action's system prompt: pin the
/// OUTPUT LANGUAGE (so a result never drifts away from the user's configured
/// default language — e.g. Japanese text staying Japanese after proofreading)
/// and, optionally, the global writing tone.
fn output_constraints(language: Option<&str>, tone: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(lang) = language.map(str::trim).filter(|l| !l.is_empty()) {
        s.push_str(&format!(
            " IMPORTANT: write your ENTIRE output in {lang}, regardless of the input language — \
             do not switch to any other language."
        ));
    }
    if let Some(t) = tone.map(str::trim).filter(|t| !t.is_empty()) {
        s.push_str(&format!(" Adopt a {t} writing tone."));
    }
    s
}

// Analysis graph types (AnalysisNode/Edge/Result) live in `models.rs` so they
// can be persisted on `Document`.

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

/// Build the system prompt for a one-click action.
fn action_system(req: &AiRequest) -> AppResult<String> {
    let lang = req.output_language.as_deref();
    let tone = req.tone.as_deref();

    let system = match req.action.as_str() {
        "translate" => {
            let target = req
                .target_language
                .clone()
                .unwrap_or_else(|| "English".to_string());
            format!(
                "You are an expert academic translator. Translate the user's target paragraph into {target}, \
                 preserving meaning, terminology and an academic tone. Use the surrounding context only to \
                 disambiguate; do not translate or repeat the context. Output ONLY the translated paragraph, \
                 with no preamble, notes, or quotation marks.{}",
                // Translate already names its target language; only the tone is
                // an extra constraint here.
                output_constraints(None, tone)
            )
        }
        "proofread" => {
            // Explicit per-action style wins; otherwise fall back to the global
            // writing tone, then to a scholarly default.
            let style = req
                .style
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or_else(|| tone.map(str::trim).filter(|t| !t.is_empty()))
                .unwrap_or("scholarly and academic");
            format!(
                "You are a meticulous copy-editor. Correct spelling, grammar and punctuation, improve \
                 clarity and concision, and adjust the writing toward a {style} style, while preserving \
                 the author's meaning. Use the surrounding context only for consistency. \
                 Output ONLY the revised paragraph, with no preamble, explanations, or quotation marks.{}",
                output_constraints(lang, None)
            )
        }
        "summarize" => format!(
            "You are an expert academic editor. Write a single concise sentence summarizing the \
             target paragraph, suitable as metadata. Output ONLY that sentence.{}",
            output_constraints(lang, None)
        ),
        "expand" => format!(
            "You are an academic writing assistant. Expand and develop the target paragraph: add \
             supporting sentences, elaboration, and smooth transitions so it reads more thoroughly, while \
             preserving the original meaning and tone. Do not introduce unrelated claims or \
             fabricated facts. Use the surrounding context only for coherence; do not repeat it. Output \
             ONLY the expanded paragraph, with no preamble, explanation, or quotation marks.{}",
            output_constraints(lang, tone)
        ),
        "detailed" => format!(
            "You are an academic writing assistant. Rewrite the target paragraph in greater \
             detail: turn general statements into specific, concrete ones and add clarifying explanation, \
             while preserving the original meaning and tone. Do not invent false facts, data, \
             or citations. Use the surrounding context only for coherence; do not repeat it. Output ONLY \
             the revised paragraph, with no preamble, explanation, or quotation marks.{}",
            output_constraints(lang, tone)
        ),
        "concentrate" => format!(
            "You are an academic writing assistant. Condense the target paragraph: remove \
             redundancy and wordiness and tighten the phrasing so it is more concise, while keeping all \
             key information and preserving the original meaning and tone. Use the surrounding \
             context only for coherence; do not repeat it. Output ONLY the condensed paragraph, with no \
             preamble, explanation, or quotation marks.{}",
            output_constraints(lang, tone)
        ),
        "focus" => format!(
            "You are an academic writing assistant. Sharpen the target paragraph so it centers \
             clearly on its main point: cut tangential or digressive material and keep the core argument, \
             while preserving the original meaning and tone. Use the surrounding context only \
             for coherence; do not repeat it. Output ONLY the focused paragraph, with no preamble, \
             explanation, or quotation marks.{}",
            output_constraints(lang, tone)
        ),
        "harmonize" => format!(
            "You are an academic writing assistant. Revise the target paragraph so it connects smoothly \
             and logically with the preceding and following paragraphs: smooth abrupt transitions, align \
             terminology, tense and voice with the neighbours, and remove repetition of what they already \
             say — while preserving the paragraph's own meaning. Use the surrounding context as the \
             reference for coherence; do NOT merge it in or repeat it. Output ONLY the revised paragraph, \
             with no preamble, explanation, or quotation marks.{}",
            output_constraints(lang, tone)
        ),
        "custom" => {
            let base = req
                .instruction
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    "You are a helpful academic writing assistant. Improve the target paragraph.".to_string()
                });
            format!("{base}{}", output_constraints(lang, tone))
        }
        other => return Err(AppError::Other(format!("Unknown AI action: '{other}'"))),
    };
    Ok(system)
}

/// Assemble the user message (surrounding context + the target paragraph).
fn action_user(req: &AiRequest) -> String {
    format!("{}[Target paragraph]\n{}", context_block(req), req.text.trim())
}

/// Run a one-click text action (translate / proofread / summarize / custom).
pub async fn run_action(config: &LlmConfig, req: &AiRequest) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());
    let system = action_system(req)?;
    let user = action_user(req);
    provider.complete(&system, &user).await
}

/// Streaming variant of `run_action`: `on_delta` receives the FULL accumulated
/// text each time new content arrives; the final text is returned.
pub async fn run_action_stream<F: FnMut(&str)>(
    config: &LlmConfig,
    req: &AiRequest,
    on_delta: F,
) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());
    let system = action_system(req)?;
    let user = action_user(req);
    provider.complete_stream(&system, &user, on_delta).await
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

/// Generate an image from a text prompt using the configured image model.
/// Returns a data URL (or remote URL). Fails loud (with a response hint) if no
/// image is found, rather than silently producing a blank image.
pub async fn generate_image(config: &LlmConfig, prompt: &str) -> AppResult<String> {
    let client = reqwest::Client::new();
    let res = client
        .post(&config.endpoint)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("HTTP-Referer", "https://github.com/kumeS/AIX_Text_Editor")
        .header("X-Title", "aixTextEditor")
        .json(&json!({
            "model": config.model,
            "messages": [{ "role": "user", "content": prompt }],
            // Ask image-capable models (e.g. Gemini "Nano Banana") for image output.
            "modalities": ["image", "text"]
        }))
        .send()
        .await?;

    let status = res.status();
    let body: serde_json::Value = res.json().await?;
    if !status.is_success() {
        let provider_msg = body["error"]["message"]
            .as_str()
            .or_else(|| body["error"].as_str())
            .unwrap_or("unknown error");
        return Err(AppError::Network(format!(
            "Image API {}: {provider_msg}",
            status.as_u16()
        )));
    }

    if let Some(url) = extract_image_url(&body) {
        return Ok(url);
    }
    // Fail loud with a short hint of the response shape (base64 can be huge).
    let hint: String = body.to_string().chars().take(300).collect();
    Err(AppError::Network(format!(
        "The image model returned no image. Verify '{}' is an image-generation model \
         on openrouter.ai/models. Response (truncated): {hint}",
        config.model
    )))
}

/// Best-effort extraction of a generated image URL from various response shapes.
fn extract_image_url(body: &serde_json::Value) -> Option<String> {
    let msg = &body["choices"][0]["message"];

    // 1) OpenRouter image-capable chat: message.images[].image_url.url
    if let Some(images) = msg["images"].as_array() {
        for img in images {
            for path in [&img["image_url"]["url"], &img["url"]] {
                if let Some(u) = path.as_str() {
                    if !u.is_empty() {
                        return Some(u.to_string());
                    }
                }
            }
        }
    }
    // 2) message.content is a string containing a data: URL
    if let Some(content) = msg["content"].as_str() {
        if let Some(u) = find_data_url(content) {
            return Some(u);
        }
    }
    // 3) message.content is an array of parts ({type:"image_url", image_url:{url}})
    if let Some(parts) = msg["content"].as_array() {
        for p in parts {
            if let Some(u) = p["image_url"]["url"].as_str() {
                if !u.is_empty() {
                    return Some(u.to_string());
                }
            }
        }
    }
    // 4) OpenAI images-API style: data[0].url / data[0].b64_json
    if let Some(first) = body["data"].as_array().and_then(|a| a.first()) {
        if let Some(u) = first["url"].as_str() {
            if !u.is_empty() {
                return Some(u.to_string());
            }
        }
        if let Some(b64) = first["b64_json"].as_str() {
            if !b64.is_empty() {
                return Some(format!("data:image/png;base64,{b64}"));
            }
        }
    }
    None
}

fn find_data_url(s: &str) -> Option<String> {
    let idx = s.find("data:image/")?;
    let rest = &s[idx..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ')' || c == '"' || c == '\'')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

const DRAFT_SYSTEM_PROMPT: &str =
    "You are an academic writing assistant. Write a coherent, well-structured first draft on the user's \
     theme. Organise it with Markdown ATX headings to show the document structure — '#' for chapter-level \
     headings, '##' for sections, '###' for subsections — and write the body as clear prose paragraphs \
     separated by blank lines. Order the material logically (e.g. introduction, development, conclusion). \
     Do NOT restate the theme verbatim as the very first line, and do NOT use bullet lists, tables, code \
     fences, or any commentary — output ONLY the draft itself (headings and paragraphs).";

/// Stream a draft, invoking `on_delta` with the full accumulated text as it grows.
/// `target_words` sets an approximate length; `output_language`/`tone` pin the
/// language and voice; `reference` is optional supporting material (pasted text,
/// fetched URL/PDF text) the draft should draw on.
pub async fn generate_draft_stream<F: FnMut(&str)>(
    config: &LlmConfig,
    theme: &str,
    target_words: Option<u32>,
    output_language: Option<&str>,
    tone: Option<&str>,
    reference: Option<&str>,
    on_delta: F,
) -> AppResult<String> {
    let provider = OpenRouterProvider::new(config.clone());

    let mut system = DRAFT_SYSTEM_PROMPT.to_string();
    if let Some(w) = target_words.filter(|w| *w > 0) {
        system.push_str(&format!(
            " Aim for approximately {w} words in total (within about ±20%); pace the \
             structure and depth to hit that length."
        ));
    }
    system.push_str(&output_constraints(output_language, tone));

    let user = match reference.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => {
            // Cap the reference so an over-long paste/PDF can't blow the context.
            let snippet: String = r.chars().take(12_000).collect();
            format!(
                "Theme: {}\n\n[Reference material to draw on — ground the draft in this; do not copy it verbatim]\n{}",
                theme.trim(),
                snippet
            )
        }
        None => theme.trim().to_string(),
    };

    provider.complete_stream(&system, &user, on_delta).await
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
        let snippet: String = chunk.content.chars().take(800).collect();
        if snippet.trim().is_empty() {
            continue;
        }
        listing.push_str(&format!("- id: {}\n  text: {}\n", chunk.id, snippet.replace('\n', " ")));
    }

    if listing.trim().is_empty() {
        return Ok(AnalysisResult { nodes: vec![], edges: vec![] });
    }

    let system = "You are a discourse-analysis engine for academic writing. Given a list of paragraphs \
         (each with an id), build a relationship network at TWO levels.\n\
         1) PARAGRAPH nodes: one per provided paragraph. Set kind=\"paragraph\", id = the paragraph id, \
         label = a 3-6 word topic, summary = one sentence.\n\
         2) SENTENCE nodes: split each paragraph into its sentences and create one node per sentence. Set \
         kind=\"sentence\", parent = the owning paragraph id, id = \"<paragraphId>#s<n>\" (n starts at 1 per \
         paragraph), label = a 3-6 word gist, summary = the sentence text.\n\
         Then add EDGES describing the logical relationship between nodes — between paragraphs, between \
         sentences, and across levels where relevant. Each edge MUST set \"relation\" to the relationship \
         type as a property (e.g. cause, effect, evidence, claim, elaboration, contrast, condition, \
         example, definition, sequence).\n\
         Respond with STRICT JSON only, no markdown, of the exact shape: \
         {\"nodes\":[{\"id\":\"...\",\"kind\":\"paragraph|sentence\",\"parent\":\"<paragraph id or omit>\",\
         \"label\":\"...\",\"summary\":\"...\"}],\
         \"edges\":[{\"source\":\"<id>\",\"target\":\"<id>\",\"relation\":\"<type>\"}]}. \
         Use ONLY the provided paragraph ids (and the \"<paragraphId>#s<n>\" form for sentences). Keep \
         labels short.";

    let user = format!("Paragraphs:\n{listing}");
    let raw = provider.complete(system, &user).await?;
    let json_str = extract_json(&raw);
    let mut result: AnalysisResult = serde_json::from_str(json_str).map_err(|e| {
        AppError::Other(format!("Could not parse analysis JSON from model: {e}"))
    })?;

    // Default any node the model left without a kind to "paragraph", and drop
    // edges whose endpoints aren't real nodes (keeps the graph consistent).
    for n in result.nodes.iter_mut() {
        if n.kind.trim().is_empty() {
            n.kind = "paragraph".to_string();
        }
    }
    let node_ids: std::collections::HashSet<&str> =
        result.nodes.iter().map(|n| n.id.as_str()).collect();
    result
        .edges
        .retain(|e| node_ids.contains(e.source.as_str()) && node_ids.contains(e.target.as_str()));

    Ok(result)
}
