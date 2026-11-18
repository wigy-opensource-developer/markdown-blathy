//! The exact on-disk path of the document the UI has open.
//!
//! Paths travel over IPC as UTF-8 JSON strings, which is lossy for a filename
//! in a legacy non-UTF-8 locale (SPEC §6). Saving in place must not go
//! through that string: the backend keeps the opened `PathBuf` and
//! `save_document` writes to it directly, so Save-after-Open overwrites the
//! right file even when the echoed display path is mangled.
//!
//! The slot is set by every command that establishes a document path
//! (`open_markdown`, `open_pending_document`, `save_markdown` as Save As) and
//! cleared by `forget_document` (New), so a plain save can never fall back to
//! a previously opened file.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct DocumentPath {
    path: Mutex<Option<PathBuf>>,
}

impl DocumentPath {
    /// Remember `path` as the document to save in place.
    pub fn set(&self, path: &Path) {
        if let Ok(mut slot) = self.path.lock() {
            *slot = Some(path.to_path_buf());
        }
    }

    /// The remembered path, cloned so the caller does not hold the lock.
    pub fn get(&self) -> Option<PathBuf> {
        self.path.lock().ok().and_then(|slot| slot.clone())
    }

    /// Forget the path (a new, untitled document).
    pub fn clear(&self) {
        if let Ok(mut slot) = self.path.lock() {
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_exact_path_until_cleared() {
        let document = DocumentPath::default();
        assert_eq!(document.get(), None);
        document.set(Path::new("/tmp/notes.md"));
        assert_eq!(document.get(), Some(PathBuf::from("/tmp/notes.md")));
        document.clear();
        assert_eq!(document.get(), None);
    }

    #[test]
    fn save_as_replaces_the_previous_path() {
        let document = DocumentPath::default();
        document.set(Path::new("/tmp/a.md"));
        document.set(Path::new("/tmp/b.md"));
        assert_eq!(document.get(), Some(PathBuf::from("/tmp/b.md")));
    }
}
