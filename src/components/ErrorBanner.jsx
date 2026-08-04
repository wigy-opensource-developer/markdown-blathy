// Transient error display (§9 of SPEC.md). Rendered on top of the app; the
// last good preview is kept when a compile error occurs.
import { memo } from "react";

function ErrorBanner({ message, onClose }) {
  if (!message) return null;
  return (
    <div id="error-banner" role="alert">
      <span className="error-message">{message}</span>
      <button type="button" aria-label="Dismiss" onClick={onClose}>
        ×
      </button>
    </div>
  );
}

export default memo(ErrorBanner);
