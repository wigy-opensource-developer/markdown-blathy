//! CLI error type and exit-code mapping.
//!
//! `Usage` problems exit with 2 (clap uses the same code); operational
//! failures exit with 1. Messages stay user-presentable and are printed to
//! stderr by `main`.

use std::fmt;
use std::io;

use markdown_blathy_core::CoreError;

#[derive(Debug)]
pub struct CliError {
    pub message: String,
    pub code: i32,
}

impl CliError {
    /// Invalid arguments or caller input (exit code 2).
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    /// An operational failure (exit code 1).
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl From<CoreError> for CliError {
    fn from(error: CoreError) -> Self {
        let message = error.to_string();
        match error {
            CoreError::InvalidInput(_) => Self::usage(message),
            CoreError::Render(_)
            | CoreError::MissingResource(_)
            | CoreError::CachePoisoned
            | CoreError::Io { .. }
            | CoreError::IoPlain { .. } => Self::failure(message),
        }
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::failure(error.to_string())
    }
}
