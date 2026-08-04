//! Thin `#[tauri::command]` wrappers (§7.4 of SPEC.md).
//!
//! All real logic lives in the plain modules (`markdown/`, `typst_engine/`)
//! which have no `tauri` imports; these functions are serialization-only
//! shims. In v1 only `render_preview` is wired (M3); the file/export commands
//! arrive in M4.

use serde::{Deserialize, Serialize};

use crate::options::RenderOptions;
use crate::typst_engine::engine;
use crate::typst_engine::template::{self, resolve_template_source};

/// Frontend → Rust payload for `render_preview` (§7.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPayload {
    pub markdown: String,
    pub options: RenderOptions,
    /// Monotonically increasing request id for the stale-response guard (§7.5).
    pub seq: u64,
}

/// Per-page SVG strings plus warnings, echoed back with the request's `seq`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOutcome {
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
    pub seq: u64,
}

/// Live preview: markdown + options → per-page SVG strings (§7.4).
///
/// Compilation is CPU-bound, so it runs on `spawn_blocking` to keep the IPC
/// handler off the async runtime's critical path. `seq` is echoed back so the
/// frontend can drop stale responses.
#[tauri::command]
pub async fn render_preview(payload: RenderPayload) -> Result<PreviewOutcome, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let source = template::build_source(&template_source, &payload.options, &payload.markdown)?;

    let outcome = tauri::async_runtime::spawn_blocking(move || engine::compile_preview(&source))
        .await
        .map_err(|e| format!("Preview task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    Ok(PreviewOutcome {
        pages: outcome.pages,
        warnings: outcome.warnings,
        seq: payload.seq,
    })
}

#[cfg(test)]
mod tests {
    use super::{PreviewOutcome, RenderPayload};
    use crate::options::{Flavor, Margins, Paper, TextAlign};

    /// Pins the IPC contract: the exact JSON the frontend `lib/api.js` sends
    /// (§8.4) must deserialize into `RenderPayload`.
    #[test]
    fn render_payload_deserializes_frontend_shape() {
        let json = r##"{
            "markdown": "# Hi",
            "options": {
                "flavor": "gfm",
                "paper": "a4",
                "margins": { "topMm": 25, "bottomMm": 25, "leftMm": 20, "rightMm": 20 },
                "textAlign": "left",
                "templatePath": null
            },
            "seq": 7
        }"##;
        let payload: RenderPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.markdown, "# Hi");
        assert_eq!(payload.seq, 7);
        assert_eq!(payload.options.flavor, Flavor::Gfm);
        assert_eq!(payload.options.paper, Paper::A4);
        assert_eq!(
            payload.options.margins,
            Margins {
                top_mm: 25.0,
                bottom_mm: 25.0,
                left_mm: 20.0,
                right_mm: 20.0,
            }
        );
        assert_eq!(payload.options.text_align, TextAlign::Left);
        assert!(payload.options.template_path.is_none());
    }

    /// Pins the IPC contract: `PreviewOutcome` serializes back to the
    /// camelCase shape the frontend reads (§7.4).
    #[test]
    fn preview_outcome_serializes_with_camel_case_and_seq() {
        let outcome = PreviewOutcome {
            pages: vec!["<svg>".into()],
            warnings: vec!["font warning".into()],
            seq: 3,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"pages\":[\"<svg>\"]"), "{json}");
        assert!(json.contains("\"warnings\":[\"font warning\"]"), "{json}");
        assert!(json.contains("\"seq\":3"), "{json}");
    }
}
