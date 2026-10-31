//! High-level operations shared by the GUI and CLI.
//!
//! The service owns the process-local engine cache and the injected template
//! directory. It never opens dialogs, launches a viewer or serializes IPC
//! payloads — those stay in the adapter. Results are typed and path/byte
//! based so the CLI can consume them without a Tauri runtime.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::engine_cache::TemplateEngineCache;
use crate::error::CoreError;
use crate::files;
use crate::markdown::convert::DocMeta;
use crate::markdown::frontmatter::{self, Patch};
use crate::options::RenderOptions;
use crate::typst_engine::engine::{self, FontOptions, PreviewOutcome};
use crate::typst_engine::template::{self, PreparedRender};

/// Input for a preview render or PDF export.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub markdown: String,
    pub options: RenderOptions,
    /// Absolute path of the document on disk, so document-relative images
    /// resolve against its folder. `None` for an unsaved/untitled document.
    pub document_path: Option<PathBuf>,
}

/// A completed preview render. The adapter adds its own transport fields
/// (the GUI's `seq`) on top.
#[derive(Debug)]
pub struct RenderResult {
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
    pub doc: DocMeta,
}

/// The engine identity used for the render, exposed so the GUI can persist a
/// prewarm hint without preparing the document twice.
#[derive(Debug, Clone)]
pub struct EngineKey {
    pub static_source: String,
    pub fonts: FontOptions,
}

/// A render result plus the engine identity that produced it.
#[derive(Debug)]
pub struct RenderDetail {
    pub result: RenderResult,
    pub engine: EngineKey,
}

/// PDF bytes plus the converter warnings that were not fatal.
#[derive(Debug)]
pub struct PdfResult {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

/// The headless application service.
pub struct Service {
    cache: Mutex<TemplateEngineCache>,
    templates_dir: Option<PathBuf>,
}

impl Service {
    /// Create a service with an explicit bundled-template directory.
    ///
    /// `None` keeps the embedded default template available; named bundled
    /// templates other than `light` then report a missing resource.
    pub fn new(templates_dir: Option<PathBuf>) -> Self {
        Self {
            cache: Mutex::new(TemplateEngineCache::new()),
            templates_dir,
        }
    }

    /// Create a service that resolves the templates next to this crate's
    /// source tree. This is the development/test fallback; adapters that ship
    /// resources should pass their resolved directory to [`Service::new`].
    pub fn with_source_templates() -> Self {
        Self::new(files::templates_dir(None))
    }

    /// The injected bundled-template directory, if any.
    pub fn templates_dir(&self) -> Option<&Path> {
        self.templates_dir.as_deref()
    }

    /// Prepare a render (resolve the template, compose its static source and
    /// convert the Markdown body). Exposed for adapters that need the engine
    /// identity for prewarming; most callers use [`Service::render`].
    pub fn prepare(&self, request: &RenderRequest) -> Result<PreparedRender, CoreError> {
        template::prepare_render(
            &request.markdown,
            &request.options,
            self.templates_dir.as_deref(),
            request.document_path.as_deref(),
        )
        .map_err(CoreError::render)
    }

    /// Compile an already-prepared render with the cached engine.
    pub fn render_prepared(&self, prepared: &PreparedRender) -> Result<PreviewOutcome, CoreError> {
        let mut cache = self.cache.lock().map_err(|_| CoreError::CachePoisoned)?;
        let engine = cache
            .engine_for(&prepared.static_source, &prepared.fonts)
            .map_err(CoreError::render)?;
        engine::compile_preview_with_inputs(engine, &prepared.inputs)
            .map_err(|e| CoreError::render(e.to_string()))
    }

    /// Render Markdown to per-page SVGs plus warnings and document metadata.
    pub fn render(&self, request: &RenderRequest) -> Result<RenderResult, CoreError> {
        self.render_detailed(request).map(|detail| detail.result)
    }

    /// Render and also return the engine identity for a prewarm hint.
    pub fn render_detailed(&self, request: &RenderRequest) -> Result<RenderDetail, CoreError> {
        let prepared = self.prepare(request)?;
        let outcome = self.render_prepared(&prepared)?;
        Ok(RenderDetail {
            result: RenderResult {
                pages: outcome.pages,
                warnings: merged_warnings(prepared.warnings.clone(), outcome.warnings),
                doc: prepared.doc.clone(),
            },
            engine: EngineKey {
                static_source: prepared.static_source,
                fonts: prepared.fonts,
            },
        })
    }

    /// Compile Markdown to PDF bytes. The caller decides whether to write a
    /// file, stream stdout or ask the user for a path.
    pub fn render_pdf(&self, request: &RenderRequest) -> Result<Vec<u8>, CoreError> {
        self.render_pdf_detailed(request).map(|result| result.bytes)
    }

    /// Compile Markdown to PDF bytes and return the non-fatal warnings too.
    pub fn render_pdf_detailed(&self, request: &RenderRequest) -> Result<PdfResult, CoreError> {
        let prepared = self.prepare(request)?;
        let mut cache = self.cache.lock().map_err(|_| CoreError::CachePoisoned)?;
        let engine = cache
            .engine_for(&prepared.static_source, &prepared.fonts)
            .map_err(CoreError::render)?;
        let bytes = engine::compile_pdf_with_inputs(engine, &prepared.inputs)
            .map_err(|e| CoreError::render(e.to_string()))?;
        Ok(PdfResult {
            bytes,
            warnings: prepared.warnings,
        })
    }

    /// Read a Markdown document from an explicit path. Never opens a dialog.
    pub fn read_document(&self, path: &Path) -> Result<String, CoreError> {
        std::fs::read_to_string(path)
            .map_err(|e| CoreError::io("Failed to read file", path.to_path_buf(), e))
    }

    /// Write a document atomically to an explicit path. Never opens a dialog.
    pub fn write_document(&self, path: &Path, content: &[u8]) -> Result<(), CoreError> {
        files::write_atomically(path, content)
            .map_err(|e| CoreError::io("Failed to write file", path.to_path_buf(), e))
    }

    /// Rewrite the document's front matter with `patch` applied, preserving
    /// unmanaged keys and the body.
    pub fn update_front_matter(&self, markdown: &str, patch: &Patch) -> Result<String, CoreError> {
        Ok(frontmatter::apply_patch(markdown, patch))
    }

    /// Effective document metadata using the fresh-install render options as
    /// the base for values the document itself does not set.
    pub fn read_front_matter(&self, markdown: &str) -> Result<DocMeta, CoreError> {
        self.read_front_matter_with_options(markdown, &RenderOptions::frontend_defaults())
    }

    /// Effective document metadata with explicit base options.
    pub fn read_front_matter_with_options(
        &self,
        markdown: &str,
        options: &RenderOptions,
    ) -> Result<DocMeta, CoreError> {
        template::render_inputs(markdown, options)
            .map(|(_, doc, _)| doc)
            .map_err(CoreError::render)
    }

    /// Read a custom template, compose it over the embedded defaults, probe
    /// compile it and prime the engine cache for the next render.
    pub fn validate_template(&self, path: &Path, options: &RenderOptions) -> Result<(), CoreError> {
        let user_source = std::fs::read_to_string(path).map_err(|e| {
            CoreError::io(
                format!("Failed to read template at {}", path.display()),
                path.to_path_buf(),
                e,
            )
        })?;
        let template_source = format!("{}\n\n{user_source}", template::DEFAULT_TEMPLATE);
        let static_source =
            template::build_static_source(&template_source, options).map_err(CoreError::render)?;
        let path_string = path.to_string_lossy();
        let fonts = engine::font_options_for_source(&path_string, &user_source);

        let engine = template::build_and_probe(&static_source, &fonts).map_err(|e| {
            CoreError::render(format!("Template at {} rejected: {e}", path.display()))
        })?;
        let mut cache = self.cache.lock().map_err(|_| CoreError::CachePoisoned)?;
        cache.prime(&static_source, &fonts, engine);
        Ok(())
    }

    /// List the bundled template ids, with the default `light` first.
    pub fn list_templates(&self) -> Result<Vec<String>, CoreError> {
        let dir = self
            .templates_dir
            .as_deref()
            .ok_or_else(|| CoreError::missing("Bundled templates are not available"))?;
        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|e| {
            CoreError::io(
                format!("Failed to list templates at {}", dir.display()),
                dir.to_path_buf(),
                e,
            )
        })? {
            let entry =
                entry.map_err(|e| CoreError::io_plain("Failed to read template entry", e))?;
            if entry.path().join("main.typ").is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        if let Some(pos) = names.iter().position(|name| name == "light") {
            let light = names.remove(pos);
            names.insert(0, light);
        }
        Ok(names)
    }

    /// Build (or reuse) an engine for a known static source. Used by the GUI
    /// startup prewarm; a miss only costs the normal rebuild on first render.
    pub fn warm_engine(&self, static_source: &str, fonts: &FontOptions) -> Result<(), CoreError> {
        let mut cache = self.cache.lock().map_err(|_| CoreError::CachePoisoned)?;
        cache
            .engine_for(static_source, fonts)
            .map(|_| ())
            .map_err(CoreError::render)
    }
}

/// The UI shows converter warnings (e.g. duplicate heading IDs) and engine
/// warnings (e.g. missing fonts) together (§3): converter first, engine after.
fn merged_warnings(mut converter: Vec<String>, engine: Vec<String>) -> Vec<String> {
    converter.extend(engine);
    converter
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Flavor, Margins, Paper, TextAlign};

    fn service() -> Service {
        Service::with_source_templates()
    }

    fn request(markdown: &str) -> RenderRequest {
        RenderRequest {
            markdown: markdown.to_string(),
            options: RenderOptions {
                flavor: Flavor::Commonmark,
                paper: Paper::A4,
                margins: Margins {
                    top_mm: 20.0,
                    bottom_mm: 20.0,
                    left_mm: 20.0,
                    right_mm: 20.0,
                },
                text_align: TextAlign::Left,
                template_name: None,
                template_path: None,
                heading_ids: Default::default(),
            },
            document_path: None,
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "markdown-blathy-core-service-{}-{tag}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn render_returns_pages_warnings_and_doc_meta() {
        let rendered = service()
            .render(&request("---\ntitle: Service\n---\n# Heading"))
            .expect("render must succeed");
        assert!(!rendered.pages.is_empty());
        assert!(rendered.pages[0].contains("<svg"));
        assert_eq!(rendered.doc.title.as_deref(), Some("Service"));
    }

    #[test]
    fn render_pdf_returns_pdf_bytes() {
        let pdf = service()
            .render_pdf(&request("# PDF"))
            .expect("PDF compile must succeed");
        assert!(pdf.starts_with(b"%PDF"), "missing PDF magic");
        assert!(!pdf.is_empty());
    }

    #[test]
    fn read_and_write_document_are_path_based() {
        let dir = temp_dir("io");
        let path = dir.join("notes.md");
        let svc = service();

        svc.write_document(&path, b"# Notes").unwrap();
        assert_eq!(svc.read_document(&path).unwrap(), "# Notes");
    }

    #[test]
    fn front_matter_reads_and_updates() {
        let svc = service();
        let patch = Patch {
            title: Some("Updated".into()),
            ..Patch::default()
        };
        let updated = svc.update_front_matter("# Body", &patch).unwrap();
        assert!(updated.contains("title: Updated"), "{updated}");

        let meta = svc.read_front_matter(&updated).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Updated"));
    }

    #[test]
    fn list_templates_puts_light_first() {
        let names = service().list_templates().unwrap();
        assert_eq!(names.first().map(String::as_str), Some("light"));
        assert!(names.contains(&"dark".to_string()));
    }

    #[test]
    fn validate_template_accepts_the_bundled_light_template() {
        let svc = service();
        let templates = svc.templates_dir().expect("source templates exist");
        let light = templates.join("light").join("main.typ");
        let options = RenderOptions::frontend_defaults();

        svc.validate_template(&light, &options)
            .expect("bundled light template must validate");
    }

    #[test]
    fn list_templates_reports_missing_resources() {
        let err = Service::new(None).list_templates().unwrap_err();
        assert!(matches!(err, CoreError::MissingResource(_)), "{err:?}");
    }
}
