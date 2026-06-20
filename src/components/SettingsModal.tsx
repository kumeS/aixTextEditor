// Settings dialog: OpenRouter endpoint, model, default translation language,
// temperature, and the API key (stored in the OS keychain via Rust — never
// echoed back to the frontend).

import { useEffect, useState } from "react";
import { api } from "../api";
import { useStore } from "../store";
import type { Settings } from "../types";
import { CloseIcon } from "./icons";

const DEFAULTS: Settings = {
  endpoint: "https://openrouter.ai/api/v1/chat/completions",
  model: "google/gemma-2-9b-it:free",
  defaultTargetLanguage: "English",
  temperature: 0.3,
};

export default function SettingsModal() {
  const open = useStore((s) => s.settingsOpen);
  const closeSettings = useStore((s) => s.closeSettings);
  const storedSettings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  const hasApiKey = useStore((s) => s.hasApiKey);
  const setHasApiKey = useStore((s) => s.setHasApiKey);
  const notify = useStore((s) => s.notify);

  const [form, setForm] = useState<Settings>(storedSettings ?? DEFAULTS);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setForm(storedSettings ?? DEFAULTS);
      setApiKey("");
    }
  }, [open, storedSettings]);

  if (!open) return null;

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const save = async () => {
    setSaving(true);
    try {
      await api.saveSettings(form);
      setSettings(form);
      if (apiKey.trim()) {
        await api.setApiKey(apiKey.trim());
        setHasApiKey(true);
      }
      notify("Settings saved.", "success");
      closeSettings();
    } catch (e) {
      notify(typeof e === "string" ? e : String(e), "error");
    } finally {
      setSaving(false);
    }
  };

  const clearKey = async () => {
    try {
      await api.deleteApiKey();
      setHasApiKey(false);
      setApiKey("");
      notify("API key removed from keychain.", "success");
    } catch (e) {
      notify(typeof e === "string" ? e : String(e), "error");
    }
  };

  const field = "w-full rounded-md border border-gray-300 px-3 py-2 text-sm outline-none focus:border-accent";
  const labelCls = "mb-1 block text-sm font-medium text-ink-soft";

  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/30 p-4"
      onMouseDown={closeSettings}
    >
      <div
        className="w-full max-w-lg rounded-xl bg-white p-6 shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-ink">Settings</h2>
          <button
            onClick={closeSettings}
            className="text-ink-faint hover:text-ink"
            aria-label="Close"
          >
            <CloseIcon />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className={labelCls}>OpenRouter API key</label>
            <input
              type="password"
              autoComplete="off"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={hasApiKey ? "•••••••••• (saved in keychain)" : "sk-or-..."}
              className={field}
            />
            <div className="mt-1 flex items-center justify-between">
              <span className="text-xs text-ink-faint">
                {hasApiKey
                  ? "A key is stored securely in your OS keychain."
                  : "Stored in your OS keychain — never written to disk in plaintext."}
              </span>
              {hasApiKey && (
                <button
                  onClick={clearKey}
                  className="text-xs text-red-500 hover:underline"
                >
                  Remove key
                </button>
              )}
            </div>
          </div>

          <div>
            <label className={labelCls}>Endpoint URL</label>
            <input
              value={form.endpoint}
              onChange={(e) => update("endpoint", e.target.value)}
              className={field}
              placeholder={DEFAULTS.endpoint}
            />
            <p className="mt-1 text-xs text-ink-faint">
              Any OpenAI-compatible chat-completions endpoint (OpenRouter, a local
              Ollama bridge, etc.).
            </p>
          </div>

          <div>
            <label className={labelCls}>Model</label>
            <input
              value={form.model}
              onChange={(e) => update("model", e.target.value)}
              className={field}
              placeholder={DEFAULTS.model}
            />
            <p className="mt-1 text-xs text-ink-faint">
              e.g. a free model like <code>google/gemma-2-9b-it:free</code>, or a
              paid one like <code>anthropic/claude-3.5-sonnet</code>. Check
              openrouter.ai/models for current ids.
            </p>
          </div>

          <div className="flex gap-4">
            <div className="flex-1">
              <label className={labelCls}>Default translation language</label>
              <input
                value={form.defaultTargetLanguage}
                onChange={(e) => update("defaultTargetLanguage", e.target.value)}
                className={field}
              />
            </div>
            <div className="w-40">
              <label className={labelCls}>
                Temperature: {form.temperature.toFixed(1)}
              </label>
              <input
                type="range"
                min={0}
                max={1}
                step={0.1}
                value={form.temperature}
                onChange={(e) => update("temperature", Number(e.target.value))}
                className="mt-3 w-full accent-accent"
              />
            </div>
          </div>
        </div>

        <div className="mt-6 flex justify-end gap-2">
          <button
            onClick={closeSettings}
            className="rounded-md px-4 py-2 text-sm text-ink-soft hover:bg-gray-100"
          >
            Cancel
          </button>
          <button
            onClick={save}
            disabled={saving}
            className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent-soft disabled:opacity-50"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
