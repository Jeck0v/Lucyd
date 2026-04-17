//! Static UI asset serving.
//!
//! The compiled React UI lives in `ui/dist/` at the repository root
//! and is embedded into the `lucy-core` binary at compile time via
//! [`rust_embed::RustEmbed`]. The [`serve_asset`] handler resolves
//! incoming paths against that embedded bundle and falls back to
//! `index.html` for any unknown path so the SPA router can take over.

use axum::{
    body::Body,
    extract::Path,
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

/// Path to the fallback HTML file served for all unknown SPA routes.
const INDEX_HTML: &str = "index.html";

/// MIME type used for the HTML fallback response.
const HTML_MIME: &str = "text/html; charset=utf-8";

/// MIME type used when a file extension is unknown or missing.
const OCTET_STREAM_MIME: &str = "application/octet-stream";

/// Message returned when the embedded UI bundle is missing entirely.
const UI_NOT_BUILT_MSG: &str = "UI not built";

/// Embeds the compiled React UI from `ui/dist/` into the binary.
///
/// The folder path is relative to the crate root (`lucy-core/`).
/// Run `cargo xtask build-ui` before building `lucy-core` in a
/// production profile to guarantee the folder exists.
///
/// Note: in debug builds the folder is optional — if `ui/dist/`
/// is missing, `rust-embed` will embed an empty asset set and
/// [`serve_asset`] will respond with `404 UI not built`. This keeps
/// the inner development loop fast because contributors can iterate
/// on the Rust runtime without first running the UI toolchain.
#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct UiAssets;

/// Serves `index.html` directly — used for the `/docs` and `/docs/` root routes
/// which Axum's `{*path}` wildcard does not match (it requires at least one segment).
pub async fn serve_index() -> Response {
    match UiAssets::get(INDEX_HTML) {
        Some(index) => build_html_response(index.data.as_ref()),
        None => (StatusCode::NOT_FOUND, UI_NOT_BUILT_MSG).into_response(),
    }
}

/// Serves a static asset from the embedded UI bundle.
///
/// Falls back to `index.html` for unrecognised paths so the SPA's
/// client-side router can handle deep links. Returns `404` only when
/// even the fallback is missing, which typically means the UI was
/// never built.
pub async fn serve_asset(Path(path): Path<String>) -> Response {
    // Strip leading slash if present so asset lookup keys match the
    // bundle layout produced by Vite / the UI toolchain.
    let asset_path = path.trim_start_matches('/');

    // Defense-in-depth: explicitly reject paths containing `..` components.
    // RustEmbed mitigates traversal by working against a closed, compile-time
    // embedded asset set (not the filesystem), but we reject such paths
    // explicitly so the protection does not silently rely on that property.
    if asset_path.contains("..") {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }

    if let Some(content) = UiAssets::get(asset_path) {
        return build_asset_response(asset_path, content.data.as_ref());
    }

    // Fall back to index.html so the SPA router can take over.
    match UiAssets::get(INDEX_HTML) {
        Some(index) => build_html_response(index.data.as_ref()),
        None => (StatusCode::NOT_FOUND, UI_NOT_BUILT_MSG).into_response(),
    }
}

/// Builds a `200 OK` response for a concrete asset file.
///
/// Content-Type is derived from the file extension; unknown
/// extensions fall back to [`OCTET_STREAM_MIME`] so the browser can
/// still download the resource safely.
fn build_asset_response(asset_path: &str, bytes: &[u8]) -> Response {
    let mime = mime_from_extension(asset_path);
    let header_value = HeaderValue::from_str(mime).unwrap_or_else(|_| {
        // Mime tables only contain ASCII, so this branch should be
        // unreachable in practice; we still handle it defensively
        // rather than panicking.
        HeaderValue::from_static(OCTET_STREAM_MIME)
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, header_value)
        .body(Body::from(bytes.to_vec()))
        .unwrap_or_else(|_| {
            // Body construction from a fully-owned Vec<u8> cannot
            // fail in practice; we keep a safe fallback anyway.
            (StatusCode::INTERNAL_SERVER_ERROR, "response build failure").into_response()
        })
}

/// Builds a `200 OK` HTML response for the SPA fallback document.
fn build_html_response(bytes: &[u8]) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static(HTML_MIME))
        .body(Body::from(bytes.to_vec()))
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "response build failure").into_response()
        })
}

/// Returns a best-effort MIME type for a given asset path based on
/// its extension. Kept intentionally tiny to avoid pulling in a full
/// MIME database for a handful of common UI asset types.
fn mime_from_extension(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("map") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        _ => OCTET_STREAM_MIME,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_from_extension_returns_expected_types() {
        assert_eq!(
            mime_from_extension("index.html"),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            mime_from_extension("app.js"),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(mime_from_extension("style.css"), "text/css; charset=utf-8");
        assert_eq!(mime_from_extension("data.json"), "application/json");
        assert_eq!(mime_from_extension("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn mime_from_extension_falls_back_to_octet_stream() {
        assert_eq!(mime_from_extension("weird.xyz"), OCTET_STREAM_MIME);
        assert_eq!(mime_from_extension("noextension"), OCTET_STREAM_MIME);
    }

    #[test]
    fn constants_are_stable() {
        // Changing these constants is a breaking change for the UI
        // bundle; pin them so regressions surface in CI immediately.
        assert_eq!(INDEX_HTML, "index.html");
        assert_eq!(UI_NOT_BUILT_MSG, "UI not built");
    }
}
