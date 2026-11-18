// Post-export compile warnings (M9 task 5). The preview's warnings live in
// the status bar, but an export is a one-shot action: the PDF is already on
// disk, so its warnings are shown in a panel the user dismisses explicitly
// (an error banner would wrongly imply the export failed).
import { memo } from "react";
import { fileBasename } from "../lib/title.js";

function ExportWarnings({ notice, onClose }) {
  if (!notice) return null;
  const count = notice.warnings.length;
  return (
    <div id="export-warnings" role="status" aria-live="polite">
      <div className="export-warnings-head">
        <span>
          Exported <strong>{fileBasename(notice.path)}</strong> with {count}{" "}
          warning{count === 1 ? "" : "s"}.
        </span>
        <button
          type="button"
          aria-label="Dismiss export warnings"
          onClick={onClose}
        >
          ×
        </button>
      </div>
      <ul>
        {notice.warnings.map((warning, i) => (
          <li key={i}>{warning}</li>
        ))}
      </ul>
    </div>
  );
}

export default memo(ExportWarnings);
