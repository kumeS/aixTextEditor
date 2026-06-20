// Visual-line caret detection for a <textarea>.
//
// A textarea soft-wraps long paragraphs, so "is the caret on the first/last
// line?" cannot be answered from the character offset alone (one logical line
// can occupy several visual rows). We mirror the textarea into a hidden div with
// the same width/typography, place a marker at the caret, and compare its
// position against the content box. This works regardless of wrapping and is
// used to move focus between chunks when Up/Down is pressed at a paragraph edge.

const COPIED_STYLE_PROPS = [
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontStyle",
  "fontVariant",
  "letterSpacing",
  "lineHeight",
  "textTransform",
  "textIndent",
  "textAlign",
  "wordSpacing",
  "tabSize",
] as const;

export interface CaretVerticalEdge {
  atFirstLine: boolean;
  atLastLine: boolean;
}

/** Whether the textarea caret currently sits on the first / last visual line. */
export function caretVerticalEdge(ta: HTMLTextAreaElement): CaretVerticalEdge {
  const computed = window.getComputedStyle(ta);
  const mirror = document.createElement("div");
  const s = mirror.style;

  s.position = "absolute";
  s.left = "-9999px";
  s.top = "0";
  s.visibility = "hidden";
  s.whiteSpace = "pre-wrap";
  s.overflowWrap = "break-word";
  s.wordWrap = "break-word";
  s.border = "0";
  s.boxSizing = "border-box";
  // Match the textarea's content width so the soft wrapping is identical.
  s.width = `${ta.clientWidth}px`;
  for (const prop of COPIED_STYLE_PROPS) {
    s.setProperty(
      prop.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase()),
      computed.getPropertyValue(
        prop.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase())
      )
    );
  }

  const value = ta.value;
  const caret = ta.selectionStart;
  mirror.textContent = value.slice(0, caret);
  const marker = document.createElement("span");
  marker.textContent = "​"; // zero-width space — a measurable caret stand-in
  mirror.appendChild(marker);
  // Include the trailing text so the mirror's total height matches the textarea
  // (needed for last-line detection).
  mirror.appendChild(document.createTextNode(value.slice(caret) || "​"));

  document.body.appendChild(mirror);

  const lineHeight =
    parseFloat(computed.lineHeight) ||
    parseFloat(computed.fontSize) * 1.2 ||
    16;
  const paddingTop = parseFloat(computed.paddingTop) || 0;
  const paddingBottom = parseFloat(computed.paddingBottom) || 0;

  const mirrorRect = mirror.getBoundingClientRect();
  const markerRect = marker.getBoundingClientRect();
  // Caret top measured from the top of the content box.
  const relTop = markerRect.top - mirrorRect.top - paddingTop;
  const contentHeight = mirror.scrollHeight - paddingTop - paddingBottom;

  document.body.removeChild(mirror);

  return {
    atFirstLine: relTop < lineHeight * 0.5,
    atLastLine: relTop > contentHeight - lineHeight * 1.5,
  };
}
