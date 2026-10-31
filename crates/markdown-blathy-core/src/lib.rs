//! MarkdownBláthy core: Markdown → Typst conversion, preview/PDF compilation,
//! template handling, document I/O and front matter editing.
//!
//! This crate is deliberately Tauri-free. It can be embedded by the GUI
//! adapter (`markdown-blathy-tauri`) or used by the `markdown-blathy` CLI, so
//! the conversion/render/export behavior has one implementation. Dialogs,
//! process launching, IPC serialization and GUI lifecycle state belong to the
//! adapters.
//!
//! The high-level entry point is [`Service`](service::Service).

pub mod engine_cache;
pub mod error;
pub mod files;
pub mod links;
pub mod markdown;
pub mod options;
pub mod service;
pub mod typst_engine;

pub use engine_cache::TemplateEngineCache;
pub use error::CoreError;
pub use options::RenderOptions;
pub use service::{EngineKey, PdfResult, RenderDetail, RenderRequest, RenderResult, Service};
