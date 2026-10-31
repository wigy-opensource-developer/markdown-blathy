//! Typst engine layer (§6 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports — so the whole pipeline (converter →
//! template → compile) is unit-testable with plain `cargo test`.

pub mod engine;
pub mod template;
