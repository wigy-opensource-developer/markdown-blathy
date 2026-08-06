//! The M6 engine cache (§7.5 of SPEC.md).
//!
//! One compiled `TypstEngine` per static source, rebuilt (fonts re-scanned)
//! only when the source changes. Pure Rust — no `tauri` imports beyond the
//! shared command-layer state type — so the cache behaviour is unit-testable
//! with plain `cargo test`.

use typst_as_lib::{TypstEngine, TypstTemplateMainFile};

use crate::typst_engine::engine::{self, FontOptions};

/// The M6 engine cache (§7.5): one compiled engine per static source.
///
/// Rebuilding an engine is the expensive part — the font scan runs at build
/// time — so the engine is reused across renders and recreated only when the
/// static source (the resolved template plus the toolbar options) changes.
/// The markdown body never touches it: it travels as `sys.inputs` per render.
pub struct TemplateEngineCache {
    key: String,
    engine: Option<TypstEngine<TypstTemplateMainFile>>,
}

impl TemplateEngineCache {
    pub fn new() -> Self {
        Self {
            key: String::new(),
            engine: None,
        }
    }

    /// The engine for `static_source`, rebuilt (fonts re-scanned) only when
    /// the source changed since the last call.
    pub fn engine_for(
        &mut self,
        static_source: &str,
        fonts: &FontOptions,
    ) -> Result<&TypstEngine<TypstTemplateMainFile>, String> {
        if self.key != static_source {
            // Diagnostic (stderr): a rebuild re-runs the font scan, which is
            // the ~1.5s cost the user may see on the first render after a
            // template change. Each line here should correspond to an actual
            // template/options change, never to steady-state typing.
            eprintln!(
                "[engine cache] rebuild: static source changed ({} -> {} chars)",
                self.key.len(),
                static_source.len()
            );
            self.engine =
                Some(engine::build_engine(static_source, fonts).map_err(|e| e.to_string())?);
            self.key = static_source.to_string();
        }
        Ok(self.engine.as_ref().expect("engine built just above"))
    }

    /// Store a ready-built engine under its static source. Used by template
    /// validation (§7.4): validation already scans the fonts and builds an
    /// engine, so it hands it to the cache and the next render — which uses
    /// the same static source — finds it ready instead of re-scanning.
    pub fn prime(&mut self, static_source: &str, engine: TypstEngine<TypstTemplateMainFile>) {
        self.key = static_source.to_string();
        self.engine = Some(engine);
    }
}

impl Default for TemplateEngineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::TemplateEngineCache;
    use crate::typst_engine::engine::{self, FontOptions};

    /// The main-file source an engine was built with (§7.5): the cache key is
    /// the static source, so this is what rebuild-vs-reuse is about.
    fn main_source(
        engine: &typst_as_lib::TypstEngine<typst_as_lib::TypstTemplateMainFile>,
    ) -> String {
        use typst::World;
        engine
            .with_world(|world| {
                world
                    .source(world.main())
                    .map(|s| s.text().to_owned())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }

    /// M6: the engine cache serves the engine for the current static source
    /// and rebuilds it (new main-file content) as soon as the source changes —
    /// the per-template font scan therefore runs once per source change.
    #[test]
    fn engine_cache_serves_and_rebuilds_on_source_change() {
        let mut cache = TemplateEngineCache::new();
        let fonts = FontOptions::default();

        let engine = cache.engine_for("source one", &fonts).expect("must build");
        assert_eq!(main_source(engine), "source one");

        let engine = cache.engine_for("source one", &fonts).expect("must reuse");
        assert_eq!(
            main_source(engine),
            "source one",
            "reused engine keeps its source"
        );

        let engine = cache
            .engine_for("source two", &fonts)
            .expect("must rebuild");
        assert_eq!(
            main_source(engine),
            "source two",
            "changed static source must rebuild the engine"
        );
    }

    /// M6: validation primes the cache with a ready-built engine (§7.4); a
    /// subsequent render for the same static source is served that engine
    /// instead of re-scanning fonts.
    #[test]
    fn cache_prime_serves_the_prebuilt_engine() {
        let mut cache = TemplateEngineCache::new();
        let fonts = FontOptions::default();

        let prebuilt = engine::build_engine("theme source", &fonts).expect("must build");
        cache.prime("theme source", prebuilt);
        let served = cache
            .engine_for("theme source", &fonts)
            .expect("must reuse");
        assert_eq!(
            main_source(served),
            "theme source",
            "the primed engine must be served without a rebuild"
        );
    }
}
