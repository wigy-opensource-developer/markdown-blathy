//! Consumed-once startup document state.
//!
//! `markdown-blathy gui FILE` launches this binary with a file argument. The
//! path is stored here before the builder starts and taken exactly once by the
//! frontend after mount. A consumed-once value is safer than an event that can
//! be missed during webview boot.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct StartupDocument {
    path: Mutex<Option<PathBuf>>,
}

impl StartupDocument {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path: Mutex::new(path),
        }
    }

    /// Take the pending startup path, leaving `None` behind.
    pub fn take(&self) -> Option<PathBuf> {
        self.path.lock().ok().and_then(|mut slot| slot.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_document_is_consumed_once() {
        let state = StartupDocument::new(Some(PathBuf::from("/tmp/notes.md")));
        assert_eq!(state.take(), Some(PathBuf::from("/tmp/notes.md")));
        assert_eq!(state.take(), None);
    }

    #[test]
    fn no_startup_document_stays_none() {
        let state = StartupDocument::default();
        assert_eq!(state.take(), None);
    }
}
