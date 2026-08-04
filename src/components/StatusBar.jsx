// Status bar: page count + warnings (§8.1 of SPEC.md; the full warnings list
// UI arrives in M5).
import { memo } from "react";

function StatusBar({ pageCount, warnings }) {
  return (
    <footer id="status">
      <span id="page-count">
        {pageCount > 0
          ? `${pageCount} page${pageCount === 1 ? "" : "s"}`
          : "No preview"}
      </span>
      {warnings.length > 0 && (
        <span id="warning-count" title={warnings.join("\n")}>
          {warnings.length} warning{warnings.length === 1 ? "" : "s"}
        </span>
      )}
    </footer>
  );
}

export default memo(StatusBar);
