//! External-link policy (§7.4, M4).
//!
//! Links in the preview SVG must never navigate the Tauri webview: http(s)
//! URLs are handed to the system default browser; every other scheme is
//! rejected with an error (§9). Pure — no `tauri` imports, so the policy is
//! unit-testable.

/// True when `url` may be handed to the system browser: http or https only.
pub fn is_browser_url(url: &str) -> bool {
    url::Url::parse(url)
        .map(|parsed| matches!(parsed.scheme(), "http" | "https"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_browser_url;

    #[test]
    fn allows_http_and_https() {
        assert!(is_browser_url("https://example.com/page"));
        assert!(is_browser_url("http://example.com"));
    }

    #[test]
    fn rejects_other_schemes() {
        assert!(!is_browser_url("javascript:alert(1)"));
        assert!(!is_browser_url("file:///etc/passwd"));
        assert!(!is_browser_url("ftp://example.com"));
        assert!(!is_browser_url("data:text/html,<b>x</b>"));
        assert!(!is_browser_url("mailto:a@example.com"));
    }

    #[test]
    fn rejects_unparseable_urls() {
        assert!(!is_browser_url("not a url"));
        assert!(!is_browser_url(""));
    }
}
