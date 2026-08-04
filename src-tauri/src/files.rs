//! Pure file I/O helpers (§7.4 of SPEC.md). No `tauri` imports.
//!
//! The dialog-bearing commands in `commands.rs` delegate writes here so the
//! behaviour is unit-testable without a Tauri runtime.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Write `content` to `path` atomically: write to a temporary sibling file in
/// the same directory, then rename over the target. `rename` is atomic on
/// POSIX and replaces the destination on the same volume on Windows, so a
/// reader that opens the file mid-write sees either the old or the new
/// content — never a partial write. On a failed write the temp file is
/// removed and the previous target (if any) is left untouched.
pub fn write_atomically(path: &Path, content: &[u8]) -> io::Result<()> {
    let tmp = temp_sibling(path);
    if let Err(err) = fs::write(&tmp, content) {
        // Do not leave a half-written temp file behind.
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    fs::rename(&tmp, path)
}

/// A temporary sibling of `path` (same directory) so the rename stays on one
/// filesystem, which is required for atomic replacement.
fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(&format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Fresh per-test directory under the system temp dir so parallel tests
    /// never collide.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tauri-md-editor-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_atomically_creates_target_with_content() {
        let dir = temp_dir("creates");
        let target = dir.join("doc.md");

        write_atomically(&target, b"# Hello").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "# Hello");
    }

    #[test]
    fn write_atomically_overwrites_existing_file() {
        let dir = temp_dir("overwrite");
        let target = dir.join("doc.md");
        fs::write(&target, "old content").unwrap();

        write_atomically(&target, b"new content").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
    }

    #[test]
    fn write_atomically_leaves_no_temp_files_behind() {
        let dir = temp_dir("no-temp");
        let target = dir.join("doc.md");

        write_atomically(&target, b"content").unwrap();

        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn write_atomically_errors_when_parent_directory_missing() {
        let target = std::env::temp_dir().join("tauri-md-editor-missing/child.md");

        let err = write_atomically(&target, b"x").unwrap_err();

        assert!(matches!(err.kind(), io::ErrorKind::NotFound));
        assert!(!target.exists(), "must not create the target on failure");
    }
}
