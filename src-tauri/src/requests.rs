//! Backend → frontend requests that may need the unsaved-changes guard.
//!
//! One event contract covers every "change what is on screen" signal: the
//! window manager asked to close, or a document arrived from the CLI (the
//! startup file today; a forwarded second instance later — SPEC §5, deferred
//! issues). The frontend decides and answers through commands, so the backend
//! never blocks the window event loop on a dialog.
//!
//! `CloseGate` lets exactly one close through after the frontend has decided
//! (Save or Discard): `confirm_close` arms it, the resulting `CloseRequested`
//! consumes the allowance instead of asking again.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

/// The Tauri event carrying an [`AppRequest`] payload.
pub const APP_REQUEST_EVENT: &str = "app-request";

/// A request the frontend must run through its dirty guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppRequest {
    /// Ask about unsaved changes, then call `confirm_close`.
    Close,
    /// Ask about unsaved changes, then call `open_pending_document`.
    Open,
}

/// A one-shot allowance for a window close the frontend already approved.
#[derive(Default)]
pub struct CloseGate(AtomicBool);

impl CloseGate {
    /// The frontend approved the close: let the next request through.
    pub fn allow(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// True when this close may proceed, consuming the allowance.
    pub fn take_allowance(&self) -> bool {
        self.0.swap(false, Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_requests_serialize_with_a_kind_tag() {
        assert_eq!(
            serde_json::to_string(&AppRequest::Close).unwrap(),
            r#"{"kind":"close"}"#
        );
        assert_eq!(
            serde_json::to_string(&AppRequest::Open).unwrap(),
            r#"{"kind":"open"}"#
        );
    }

    #[test]
    fn close_allowance_is_one_shot() {
        let gate = CloseGate::default();
        assert!(!gate.take_allowance(), "a fresh gate asks the frontend");
        gate.allow();
        assert!(gate.take_allowance(), "the approved close proceeds");
        assert!(
            !gate.take_allowance(),
            "the next close asks the frontend again"
        );
    }
}
