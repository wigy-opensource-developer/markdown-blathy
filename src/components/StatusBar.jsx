// Status bar: page count + warnings (§9 of SPEC.md). The `N warning(s)`
// count is a toggle that expands into a collapsible list of the full
// diagnostics; warnings never fail the render (M5). `inert` is set while the
// Document settings popover is open (App, §6.2): the bar is blurred and must
// not take clicks or screen-reader focus.
import { memo, useState } from "react";

function StatusBar({ pageCount, warnings, inert }) {
  const [warningsOpen, setWarningsOpen] = useState(false);
  const showWarnings = warnings.length > 0;

  return (
    <footer id="status" inert={inert}>
      <div id="status-row">
        <span id="page-count">
          {pageCount > 0
            ? `${pageCount} page${pageCount === 1 ? "" : "s"}`
            : "No preview"}
        </span>
        {showWarnings && (
          <button
            type="button"
            id="warning-toggle"
            aria-expanded={warningsOpen}
            onClick={() => setWarningsOpen((open) => !open)}
          >
            {warnings.length} warning{warnings.length === 1 ? "" : "s"}
            <span className="chevron" aria-hidden="true">
              {warningsOpen ? "▾" : "▸"}
            </span>
          </button>
        )}
      </div>
      {warningsOpen && (
        <ul id="warning-list">
          {warnings.map((warning, i) => (
            <li key={i}>{warning}</li>
          ))}
        </ul>
      )}
    </footer>
  );
}

export default memo(StatusBar);
