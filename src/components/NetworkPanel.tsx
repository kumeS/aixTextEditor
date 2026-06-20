// Relationship network graph (spec §3.4). Renders the LLM analysis with
// Cytoscape; tapping a node jumps to (and flashes) the matching paragraph.

import cytoscape from "cytoscape";
import { useEffect, useRef } from "react";
import { useStore } from "../store";
import { analyzeDocument } from "../aiActions";
import { CloseIcon, NetworkIcon, SpinnerIcon } from "./icons";

export default function NetworkPanel() {
  const analysis = useStore((s) => s.analysis);
  const globalBusy = useStore((s) => s.globalBusy);
  const flashChunk = useStore((s) => s.flashChunk);
  const toggleNetwork = useStore((s) => s.toggleNetwork);

  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);

  useEffect(() => {
    if (!containerRef.current || !analysis) return;

    const nodeIds = new Set(analysis.nodes.map((n) => n.id));
    const elements: cytoscape.ElementDefinition[] = [];
    for (const n of analysis.nodes) {
      elements.push({
        data: { id: n.id, label: n.label || n.summary || "·", summary: n.summary },
      });
    }
    analysis.edges.forEach((e, i) => {
      if (nodeIds.has(e.source) && nodeIds.has(e.target)) {
        elements.push({
          data: {
            id: `e${i}-${e.source}-${e.target}`,
            source: e.source,
            target: e.target,
            label: e.relation || "",
          },
        });
      }
    });

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        {
          selector: "node",
          style: {
            "background-color": "#2563eb",
            label: "data(label)",
            color: "#1f2933",
            "font-size": 11,
            "text-wrap": "wrap",
            "text-max-width": "120px",
            "text-valign": "bottom",
            "text-margin-y": 4,
            width: 18,
            height: 18,
            "border-width": 2,
            "border-color": "#bfdbfe",
          },
        },
        {
          selector: "edge",
          style: {
            width: 1.5,
            "line-color": "#cbd5e1",
            "target-arrow-color": "#cbd5e1",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            label: "data(label)",
            "font-size": 9,
            color: "#7b8794",
            "text-rotation": "autorotate",
            "text-background-color": "#ffffff",
            "text-background-opacity": 0.85,
            "text-background-padding": "1px",
          },
        },
        {
          selector: "node:active",
          style: { "overlay-color": "#2563eb", "overlay-opacity": 0.2 },
        },
      ],
      layout: { name: "cose", animate: false, padding: 24, nodeDimensionsIncludeLabels: true },
      minZoom: 0.2,
      maxZoom: 2.5,
    });

    cy.on("tap", "node", (evt) => flashChunk(evt.target.id()));
    cyRef.current = cy;

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [analysis, flashChunk]);

  const relayout = () => {
    cyRef.current
      ?.layout({ name: "cose", animate: true, padding: 24, nodeDimensionsIncludeLabels: true })
      .run();
  };

  const isEmpty = !analysis || analysis.nodes.length === 0;

  return (
    <aside className="flex h-full w-80 shrink-0 flex-col border-l border-gray-200 bg-white">
      <div className="flex items-center justify-between border-b border-gray-200 px-3 py-2">
        <div className="flex items-center gap-1.5 text-sm font-semibold text-ink">
          <NetworkIcon /> Relationships
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => void analyzeDocument()}
            className="rounded px-2 py-1 text-xs text-ink-soft hover:bg-gray-100"
            disabled={!!globalBusy}
            title="Re-analyze document"
          >
            Refresh
          </button>
          <button
            onClick={relayout}
            className="rounded px-2 py-1 text-xs text-ink-soft hover:bg-gray-100"
            disabled={isEmpty}
            title="Re-layout graph"
          >
            Re-layout
          </button>
          <button
            onClick={() => toggleNetwork(false)}
            className="rounded p-1 text-ink-faint hover:bg-gray-100 hover:text-ink"
            aria-label="Close panel"
          >
            <CloseIcon />
          </button>
        </div>
      </div>

      <div className="relative min-h-0 flex-1">
        <div ref={containerRef} className="absolute inset-0" />
        {globalBusy && (
          <div className="absolute inset-0 flex items-center justify-center bg-white/70 text-sm text-accent">
            <SpinnerIcon className="mr-2 text-accent" /> {globalBusy}
          </div>
        )}
        {isEmpty && !globalBusy && (
          <div className="absolute inset-0 flex flex-col items-center justify-center px-6 text-center text-sm text-ink-faint">
            <NetworkIcon className="mb-2 h-6 w-6" />
            No relationships yet. Click “Analyze” to extract the logical structure
            of your document.
          </div>
        )}
      </div>

      <div className="border-t border-gray-100 px-3 py-2 text-xs text-ink-faint">
        Tap a node to jump to its paragraph.
      </div>
    </aside>
  );
}
