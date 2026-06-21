// Top-level layout: toolbar over a scrollable editor, with an optional
// relationship network panel docked on the right.

import { Suspense, lazy, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import { analyzeDocument } from "./aiActions";
import {
  exportDocument,
  exportPdf,
  importDocument,
  openNative,
  saveNative,
} from "./fileActions";
import { useStore } from "./store";
import { useShortcuts } from "./useShortcuts";
import DraftModal from "./components/DraftModal";
import Editor from "./components/Editor";
import HelpModal from "./components/HelpModal";
import SelectionBar from "./components/SelectionBar";
import SettingsModal from "./components/SettingsModal";
import TabBar from "./components/TabBar";
import Toolbar from "./components/Toolbar";
import Toasts from "./components/Toasts";
import { PromptHost } from "./components/PromptModal";

// Cytoscape is heavy; load the network panel only when it is first opened.
const NetworkPanel = lazy(() => import("./components/NetworkPanel"));

function App() {
  const networkOpen = useStore((s) => s.networkOpen);
  const setSettings = useStore((s) => s.setSettings);
  const setHasApiKey = useStore((s) => s.setHasApiKey);
  const notify = useStore((s) => s.notify);

  useShortcuts();

  // Native menu → dispatch to the same handlers as the in-app toolbar.
  useEffect(() => {
    const unlisten = listen<string>("menu", async (e) => {
      const st = useStore.getState();
      switch (e.payload) {
        case "new_tab":
          st.newTab();
          break;
        case "open":
          void openNative();
          break;
        case "save":
          void saveNative();
          break;
        case "import":
          void importDocument();
          break;
        case "export_txt":
          void exportDocument("txt");
          break;
        case "export_md":
          void exportDocument("md");
          break;
        case "export_rtf":
          void exportDocument("rtf");
          break;
        case "export_pdf":
          void exportPdf();
          break;
        case "undo":
          st.undo();
          break;
        case "redo":
          st.redo();
          break;
        case "settings":
          st.openSettings();
          break;
        case "analyze":
          void analyzeDocument();
          break;
        case "draft":
          st.openDraft();
          break;
        case "help":
          st.openHelp();
          break;
      }
    });
    return () => {
      void unlisten.then((f) => f());
    };
  }, []);

  // Load persisted settings + key status on startup.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [settings, hasKey] = await Promise.all([
          api.getSettings(),
          api.hasApiKey(),
        ]);
        if (cancelled) return;
        setSettings(settings);
        setHasApiKey(hasKey);
        if (!hasKey) {
          notify(
            "Add your OpenRouter API key in Settings to enable AI features.",
            "info"
          );
        }
      } catch (e) {
        if (!cancelled) notify(typeof e === "string" ? e : String(e), "error");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [setSettings, setHasApiKey, notify]);

  return (
    <div className="flex h-full flex-col bg-white text-ink">
      <TabBar />
      <Toolbar />
      <div className="flex min-h-0 flex-1">
        <main className="min-h-0 flex-1 overflow-y-auto">
          <Editor />
        </main>
        {networkOpen && (
          <Suspense
            fallback={
              <aside className="flex h-full w-80 shrink-0 items-center justify-center border-l border-gray-200 bg-white text-sm text-ink-faint">
                Loading graph…
              </aside>
            }
          >
            <NetworkPanel />
          </Suspense>
        )}
      </div>

      <SettingsModal />
      <DraftModal />
      <HelpModal />
      <PromptHost />
      <SelectionBar />
      <Toasts />
    </div>
  );
}

export default App;
