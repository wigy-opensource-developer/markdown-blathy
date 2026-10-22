//! Engine-prewarm hint (§7.5): remember the last static source so the next
//! launch can rebuild the engine *while the boot loader is still on screen*
//! instead of under the first preview request.
//!
//! Prewarming from the frontend's fresh-install defaults would miss for every
//! returning user whose persisted options differ (a justified `text-align`
//! alone changes the composed source). The static source is the engine cache
//! key (§7.5), so caching it — plus the `FontOptions` it was built with —
//! makes the prewarm match whatever the first render will actually ask for.
//!
//! Pure file I/O (no `tauri` imports; the command layer injects the path),
//! mirroring `files.rs`: atomic writes, corrupt/missing hint = `None`.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::typst_engine::engine::FontOptions;

/// What a warmed engine needs: the composed static source (template +
/// toolbar options) and the font-loading options it was built with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrewarmHint {
    pub static_source: String,
    pub fonts: FontOptions,
}

/// The hint file inside the app's data directory.
pub fn hint_path(data_dir: &Path) -> PathBuf {
    data_dir.join("prewarm.json")
}

/// Load the remembered hint; `None` when absent or unreadable/corrupt (an
/// old-version file, a truncated write, a manual edit — all degrade to "no
/// prewarm", never to an error).
pub fn load_hint(path: &Path) -> Option<PrewarmHint> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist the hint. A source that equals what the file already holds is not
/// rewritten (renders happen per keystroke pause; the file changes only when
/// the static source actually changes). Returns Ok(()) for the no-op case;
/// write errors are for the caller to log-and-forget.
pub fn save_hint(path: &Path, hint: &PrewarmHint) -> io::Result<()> {
    if load_hint(path).as_ref() == Some(hint) {
        return Ok(());
    }
    crate::files::write_atomically(path, serde_json::to_vec(hint)?.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "markdown-blathy-prewarm-{tag}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hint(body: &str) -> PrewarmHint {
        PrewarmHint {
            static_source: format!("#article(paper: \"{body}\")"),
            fonts: FontOptions::default(),
        }
    }

    #[test]
    fn missing_hint_loads_as_none() {
        let path = temp_dir("missing").join("prewarm.json");
        assert!(load_hint(&path).is_none());
    }

    #[test]
    fn hint_round_trips() {
        let dir = temp_dir("roundtrip");
        let path = hint_path(&dir);
        let h = hint("a4");

        save_hint(&path, &h).unwrap();

        assert_eq!(load_hint(&path).unwrap(), h);
    }

    #[test]
    fn corrupt_hint_loads_as_none() {
        let dir = temp_dir("corrupt");
        let path = hint_path(&dir);
        fs::write(&path, b"{ not json").unwrap();

        assert!(load_hint(&path).is_none());
    }

    #[test]
    fn save_skips_an_identical_hint() {
        let dir = temp_dir("skip");
        let path = hint_path(&dir);
        save_hint(&path, &hint("a4")).unwrap();
        // Truncate the mtime granularity problem away: record the bytes; an
        // identical save must leave the file byte-for-byte untouched.
        let before = fs::read(&path).unwrap();

        save_hint(&path, &hint("a4")).unwrap();

        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn save_replaces_a_stale_hint() {
        let dir = temp_dir("replace");
        let path = hint_path(&dir);
        save_hint(&path, &hint("a4")).unwrap();

        save_hint(&path, &hint("us-letter")).unwrap();

        assert_eq!(load_hint(&path).unwrap(), hint("us-letter"));
    }

    #[test]
    fn save_needs_the_parent_directory_to_exist() {
        // The command layer creates the app data dir; a missing parent is an
        // error (write_atomically's contract), not a silent create.
        let path = temp_dir("noparent").join("absent").join("prewarm.json");
        assert!(save_hint(&path, &hint("a4")).is_err());
    }
}
