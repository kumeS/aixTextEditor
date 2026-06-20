// Tab strip for managing multiple open documents at once. The active tab's
// state lives in the store's top-level fields; inactive tabs are snapshots.

import { confirm } from "@tauri-apps/plugin-dialog";
import { useStore } from "../store";
import { CloseIcon, PlusIcon } from "./icons";

export default function TabBar() {
  const tabOrder = useStore((s) => s.tabOrder);
  const activeTabId = useStore((s) => s.activeTabId);
  const activeTitle = useStore((s) => s.doc.title);
  const activeDirty = useStore((s) => s.dirty);
  const inactiveTabs = useStore((s) => s.inactiveTabs);
  const switchTab = useStore((s) => s.switchTab);
  const closeTab = useStore((s) => s.closeTab);
  const newTab = useStore((s) => s.newTab);

  const titleOf = (id: string) =>
    (id === activeTabId ? activeTitle : inactiveTabs[id]?.doc.title) ||
    "Untitled";
  const dirtyOf = (id: string) =>
    id === activeTabId ? activeDirty : !!inactiveTabs[id]?.dirty;

  const onClose = async (id: string) => {
    if (dirtyOf(id)) {
      const ok = await confirm("This tab has unsaved changes. Close it?", {
        title: "Unsaved changes",
        kind: "warning",
      });
      if (!ok) return;
    }
    closeTab(id);
  };

  return (
    <div className="flex items-center gap-1 overflow-x-auto border-b border-gray-200 bg-gray-50/80 px-2 py-1">
      {tabOrder.map((id) => {
        const isActive = id === activeTabId;
        return (
          <div
            key={id}
            className={`group flex max-w-[200px] shrink-0 items-center gap-1 rounded-md px-2 py-1 text-sm ${
              isActive
                ? "bg-white text-ink shadow-sm"
                : "text-ink-faint hover:bg-white/70"
            }`}
          >
            <button
              onClick={() => switchTab(id)}
              className="truncate outline-none"
              title={titleOf(id)}
            >
              {titleOf(id)}
              {dirtyOf(id) ? " •" : ""}
            </button>
            {tabOrder.length > 1 && (
              <button
                onClick={() => void onClose(id)}
                className="shrink-0 rounded p-0.5 text-ink-faint opacity-0 transition-opacity hover:bg-gray-200 hover:text-ink group-hover:opacity-100"
                aria-label="Close tab"
                title="Close tab"
              >
                <CloseIcon className="h-3 w-3" />
              </button>
            )}
          </div>
        );
      })}
      <button
        onClick={newTab}
        className="shrink-0 rounded-md p-1 text-ink-faint hover:bg-white hover:text-ink"
        title="New tab (⌘/Ctrl+T)"
        aria-label="New tab"
      >
        <PlusIcon className="h-4 w-4" />
      </button>
    </div>
  );
}
