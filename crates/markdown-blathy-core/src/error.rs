//! Typed errors for the headless core.
//!
//! The Tauri adapter maps these to its existing `Result<T, String>` IPC
//! contract; the CLI maps the variants to exit codes and stderr messages.
//! `Display` stays user-presentable so no message text has to be rebuilt at
//! the edges.

use std::io;
use std::path::PathBuf;

/// A core operation failure.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// File-system I/O failed. `action` is a short user-facing phrase
    /// ("Failed to read file") and `path` is included.
    #[error("{action} at {path}: {source}")]
    Io {
        action: String,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// File-system I/O failed with no single path to name.
    #[error("{action}: {source}")]
    IoPlain {
        action: String,
        #[source]
        source: io::Error,
    },
    /// Template preparation or Typst compilation failed. The message already
    /// contains the user-facing diagnostics.
    #[error("{0}")]
    Render(String),
    /// Caller-provided input is invalid (empty path, conflicting options, …).
    #[error("{0}")]
    InvalidInput(String),
    /// A bundled resource (template directory, template id, …) is unavailable.
    #[error("{0}")]
    MissingResource(String),
    /// The process-local engine cache mutex was poisoned by a panic elsewhere.
    #[error("engine cache lock poisoned")]
    CachePoisoned,
}

impl CoreError {
    /// I/O failure for an operation on `path`.
    pub fn io(action: impl Into<String>, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action: action.into(),
            path: path.into(),
            source,
        }
    }

    /// I/O failure with no single path to name.
    pub fn io_plain(action: impl Into<String>, source: io::Error) -> Self {
        Self::IoPlain {
            action: action.into(),
            source,
        }
    }

    /// A render/compile/validation failure.
    pub fn render(message: impl Into<String>) -> Self {
        Self::Render(message.into())
    }

    /// Invalid caller input.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    /// A missing bundled resource.
    pub fn missing(message: impl Into<String>) -> Self {
        Self::MissingResource(message.into())
    }
}
