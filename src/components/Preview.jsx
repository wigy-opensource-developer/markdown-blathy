// Preview pane: one white "sheet" per printed page (§8.3 of SPEC.md).
//
// The SVG strings come from `render_preview` — produced by our own Typst
// compile, never from user input directly — so injecting them as DOM is safe
// (same trust boundary as the preview renderer in general).
//
// Memoized: the pages array only changes when a compile lands (debounced), so
// this component (and its large SVG DOM) is skipped on every editor keystroke
// render — keeping the controlled-editor round-trip fast.
import { memo } from "react";

function Preview({ pages, onExternalLink }) {
  // Links inside the Typst SVG pages must never navigate the webview away
  // (§9): every `<a>` click is intercepted and handed to the backend, which
  // opens http(s) in the system browser and rejects anything else with an
  // error. Same-document fragment links (`#…`) are internal — e.g. PDF-style
  // footnote/label jumps — and are left to the webview to scroll natively.
  const handleClick = (event) => {
    const anchor = event.target.closest?.("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    if (!href) return;
    if (href.startsWith("#")) return;
    event.preventDefault();
    onExternalLink(href);
  };

  if (pages.length === 0) {
    return (
      <div id="preview">
        <p className="preview-hint">
          The live preview renders here — per-page SVG from the Typst pipeline.
        </p>
      </div>
    );
  }

  return (
    <div id="preview" onClick={handleClick}>
      {pages.map((svg, i) => (
        <div
          key={i}
          className="page"
          data-page={i + 1}
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      ))}
    </div>
  );
}

export default memo(Preview);
