// A lightweight hover tooltip. Wraps a single control and shows a short
// description on hover/focus. Rendered through a portal with fixed positioning
// so it is never clipped by the editor's scroll container or the gutters.

import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface Props {
  label: string;
  children: ReactNode;
  /** Preferred side; falls back automatically near screen edges. */
  side?: "top" | "bottom";
}

export default function Tooltip({ label, children, side = "bottom" }: Props) {
  const [pos, setPos] = useState<{ x: number; y: number; place: "top" | "bottom" } | null>(
    null
  );

  const show = (e: React.MouseEvent | React.FocusEvent) => {
    // The wrapper uses display:contents, so measure the actual control.
    const el = e.currentTarget.firstElementChild as HTMLElement | null;
    const target = el ?? (e.currentTarget as HTMLElement);
    const r = target.getBoundingClientRect?.();
    if (!r) return;
    const place = side === "top" || r.bottom > window.innerHeight - 60 ? "top" : "bottom";
    const x = Math.max(48, Math.min(window.innerWidth - 48, r.left + r.width / 2));
    const y = place === "top" ? r.top - 8 : r.bottom + 8;
    setPos({ x, y, place });
  };
  const hide = () => setPos(null);

  return (
    <span
      style={{ display: "contents" }}
      onMouseOver={show}
      onMouseOut={hide}
      onFocus={show}
      onBlur={hide}
    >
      {children}
      {pos &&
        createPortal(
          <div
            role="tooltip"
            style={{
              position: "fixed",
              left: pos.x,
              top: pos.y,
              transform: `translate(-50%, ${pos.place === "top" ? "-100%" : "0"})`,
            }}
            className="pointer-events-none z-[100] max-w-[16rem] rounded-md bg-ink px-2 py-1 text-xs leading-snug text-white shadow-lg"
          >
            {label}
          </div>,
          document.body
        )}
    </span>
  );
}
