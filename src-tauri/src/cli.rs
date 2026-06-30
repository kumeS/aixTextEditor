//! Headless CLI surface — the first slice of the "Agent Experience" (AX) work
//! (report_v2 §10 T2). It lets an agent or CI script drive the document engine
//! WITHOUT the GUI, reusing the exact same pure backend functions the Tauri
//! commands call. This loop covers the offline, network-free operations
//! (inspect, convert, self-describe); the AI verbs (run/analyze/draft) and an
//! MCP wrapper are the documented next-loop extension.
//!
//! Recognized invocations (a non-subcommand first arg returns `None` so a normal
//! GUI launch — which may carry OS-injected args — is never hijacked):
//!   aixTextEditor capabilities            self-describing JSON manifest
//!   aixTextEditor info <file.aix> [--json]  document structure (ids/types/summaries)
//!   aixTextEditor export <in.aix> <out.{txt,md,rtf,pptx}>
//!   aixTextEditor help

use crate::models::Document;
use crate::{deck, fileio, pptx};

const SUBCOMMANDS: &[&str] = &["capabilities", "info", "export", "help", "--help", "-h"];

/// Returns `Some(exit_code)` if the args were a CLI invocation we handled (the
/// caller should then exit), or `None` to fall through to launching the GUI.
pub fn try_run() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first()?;
    if !SUBCOMMANDS.contains(&cmd.as_str()) {
        return None; // not ours — let the GUI start
    }
    let code = match run(cmd, &args[1..]) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("aix: {e}");
            1
        }
    };
    Some(code)
}

fn run(cmd: &str, rest: &[String]) -> Result<(), String> {
    match cmd {
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "capabilities" => {
            println!("{}", capabilities_json());
            Ok(())
        }
        "info" => {
            let path = rest.first().ok_or("info: missing <file.aix>")?;
            let as_json = rest.iter().any(|a| a == "--json");
            let doc = load(path)?;
            if as_json {
                println!("{}", info_json(&doc));
            } else {
                print_info(&doc);
            }
            Ok(())
        }
        "export" => {
            let input = rest.first().ok_or("export: missing <in.aix>")?;
            let output = rest.get(1).ok_or("export: missing <out.{txt,md,rtf,pptx}>")?;
            let doc = load(input)?;
            export(&doc, output)?;
            println!("wrote {output}");
            Ok(())
        }
        _ => {
            print_usage();
            Err(format!("unknown command '{cmd}'"))
        }
    }
}

fn load(path: &str) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {path} (expected .aix JSON): {e}"))
}

fn export(doc: &Document, output: &str) -> Result<(), String> {
    let ext = output
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "txt" | "md" | "markdown" | "rtf" => {
            fileio::export_to_path(doc, output, &ext).map_err(|e| e.to_string())
        }
        "pptx" => {
            let d = deck::document_to_deck(doc);
            let (bytes, warnings) = pptx::deck_to_pptx(&d).map_err(|e| e.to_string())?;
            std::fs::write(output, bytes).map_err(|e| format!("write {output}: {e}"))?;
            for w in warnings {
                eprintln!("warning: {w}");
            }
            Ok(())
        }
        other => Err(format!(
            "unsupported export extension '.{other}' (use txt, md, rtf, or pptx)"
        )),
    }
}

/// Self-describing manifest so a caller can discover what this build supports at
/// runtime instead of hard-coding field names (report_v2 §9 A6).
fn capabilities_json() -> String {
    serde_json::json!({
        "app": "aixTextEditor",
        "version": env!("CARGO_PKG_VERSION"),
        "aixSchemaVersion": 1,
        "aiActions": [
            "translate", "proofread", "summarize", "expand", "detailed",
            "concentrate", "focus", "harmonize", "custom"
        ],
        "chunkTypes": ["text", "heading", "diagram", "image"],
        "exportFormats": ["txt", "md", "rtf", "pdf", "pptx"],
        "cli": ["capabilities", "info", "export"]
    })
    .to_string()
}

fn info_json(doc: &Document) -> String {
    let chunks: Vec<serde_json::Value> = doc
        .chunks
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "type": c.metadata.chunk_type,
                "level": c.metadata.level,
                "summary": c.metadata.summary,
                "chars": c.content.chars().count(),
            })
        })
        .collect();
    serde_json::json!({
        "id": doc.id,
        "title": doc.title,
        "chunkCount": doc.chunks.len(),
        "hasAnalysis": doc.analysis.is_some(),
        "chunks": chunks,
    })
    .to_string()
}

fn print_info(doc: &Document) {
    let title = if doc.title.trim().is_empty() {
        "(untitled)"
    } else {
        doc.title.trim()
    };
    println!("Title: {title}");
    println!("Chunks: {}", doc.chunks.len());
    for (i, c) in doc.chunks.iter().enumerate() {
        let kind = &c.metadata.chunk_type;
        let preview: String = c.content.chars().take(60).collect();
        let preview = preview.replace('\n', " ");
        println!("  [{i:>3}] {kind:<8} {} | {preview}", c.id);
    }
}

fn print_usage() {
    eprintln!(
        "aixTextEditor — headless CLI\n\
         \n\
         USAGE:\n\
         \taixTextEditor capabilities                 self-describing JSON manifest\n\
         \taixTextEditor info <file.aix> [--json]     document structure\n\
         \taixTextEditor export <in.aix> <out.ext>    ext = txt | md | rtf | pptx\n\
         \taixTextEditor help\n\
         \n\
         Run with no arguments to launch the GUI."
    );
}
