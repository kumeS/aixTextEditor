// Typed wrappers around the Tauri command surface (src-tauri/src/commands.rs).
// All argument keys are single words, so JS camelCase maps directly to the Rust
// snake_case parameter names.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AiRequest,
  AnalysisResult,
  Document,
  DraftEvent,
  ExportFormat,
  Settings,
} from "./types";

export const api = {
  importDocument: (path: string) =>
    invoke<Document>("import_document", { path }),

  exportDocument: (document: Document, path: string, format: ExportFormat) =>
    invoke<void>("export_document", { document, path, format }),

  saveDocumentJson: (document: Document, path: string) =>
    invoke<void>("save_document_json", { document, path }),

  openDocumentJson: (path: string) =>
    invoke<Document>("open_document_json", { path }),

  getSettings: () => invoke<Settings>("get_settings"),

  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }),

  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),

  hasApiKey: () => invoke<boolean>("has_api_key"),

  deleteApiKey: () => invoke<void>("delete_api_key"),

  aiProcess: (request: AiRequest) =>
    invoke<string>("ai_process", { request }),

  /** Streaming variant; `onDelta` gets the full accumulated text, resolves with final. */
  aiProcessStream: (request: AiRequest, onDelta: (text: string) => void) => {
    const channel = new Channel<string>();
    channel.onmessage = onDelta;
    return invoke<string>("ai_process_stream", { request, onDelta: channel });
  },

  /** Stream a draft; `onEvent` fires with live snapshots then the final document. */
  aiDraftStream: (
    theme: string,
    targetWords: number | undefined,
    reference: string | undefined,
    onEvent: (e: DraftEvent) => void
  ) => {
    const channel = new Channel<DraftEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("ai_draft_stream", {
      theme,
      targetWords: targetWords ?? null,
      reference: reference ?? null,
      onEvent: channel,
    });
  },

  aiGenerateImage: (prompt: string) =>
    invoke<string>("ai_generate_image", { prompt }),

  aiGenerateDiagram: (text: string, instruction?: string) =>
    invoke<string>("ai_generate_diagram", {
      text,
      instruction: instruction ?? null,
    }),

  aiAnalyzeDocument: (document: Document) =>
    invoke<AnalysisResult>("ai_analyze_document", { document }),

  readReferenceFile: (path: string) =>
    invoke<string>("read_reference_file", { path }),

  fetchUrlText: (url: string) => invoke<string>("fetch_url_text", { url }),

  speakText: (text: string, voice?: string) =>
    invoke<void>("speak_text", { text, voice: voice ?? null }),

  stopSpeaking: () => invoke<void>("stop_speaking"),
};
