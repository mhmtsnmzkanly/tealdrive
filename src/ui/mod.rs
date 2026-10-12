pub mod assets;

use std::fs;
use std::path::{Path, PathBuf};

use axum::extract::{Path as AxumPath, State};
use axum::http::header::{
    CONTENT_SECURITY_POLICY, CONTENT_TYPE, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};

use crate::ui::assets::{APP_JS, FAVICON_SVG, INDEX_HTML, STYLE_CSS};
use crate::ws::server::{WsHelperBackend, WsServerState};

const CSP_POLICY: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; img-src 'self' data:; connect-src 'self' ws: wss:;";

fn apply_security_headers(headers: &mut HeaderMap, content_type: &'static str) {
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(
        REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP_POLICY),
    );
}

pub fn mime_for_path(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

pub fn resolve_safe_static_path(base_dir: &Path, rel_path: &str) -> Option<PathBuf> {
    let clean = rel_path.trim_start_matches('/');
    if clean.contains("..") || clean.contains('\\') || clean.contains('\0') {
        return None;
    }
    let target = base_dir.join(clean);
    let canonical_base = base_dir.canonicalize().ok()?;
    let canonical_target = target.canonicalize().ok()?;
    if canonical_target.starts_with(&canonical_base) && canonical_target.is_file() {
        Some(canonical_target)
    } else {
        None
    }
}

pub async fn serve_index<H>(State(state): State<WsServerState<H>>) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if let Some(ref static_dir) = state.config.load().server.static_dir {
        if let Some(target) = resolve_safe_static_path(static_dir, "index.html") {
            if let Ok(content) = fs::read(&target) {
                let mut response = content.into_response();
                apply_security_headers(response.headers_mut(), "text/html; charset=utf-8");
                return response;
            }
        }
    }

    static ASSET_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let version = ASSET_VERSION.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        for content in [
            INDEX_HTML,
            STYLE_CSS,
            APP_JS,
            crate::ui::assets::ACCOUNTS_JS,
            include_str!("protocol.js"),
            crate::protocol::JAVASCRIPT_CONTRACT,
        ] {
            digest.update(content.as_bytes());
        }
        format!("{:x}", digest.finalize())
    });
    let html = INDEX_HTML
        .replace("/style.css", &format!("/style.css?v={version}"))
        .replace("/app.js", &format!("/app.js?v={version}"));
    let mut response = Html(html).into_response();
    apply_security_headers(response.headers_mut(), "text/html; charset=utf-8");
    response
}

pub async fn serve_style<H>(State(state): State<WsServerState<H>>) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if let Some(ref static_dir) = state.config.load().server.static_dir {
        if let Some(target) = resolve_safe_static_path(static_dir, "style.css") {
            if let Ok(content) = fs::read(&target) {
                let mut response = content.into_response();
                apply_security_headers(response.headers_mut(), "text/css; charset=utf-8");
                return response;
            }
        }
    }

    let mut response = STYLE_CSS.into_response();
    apply_security_headers(response.headers_mut(), "text/css; charset=utf-8");
    response
}

pub async fn serve_app<H>(State(state): State<WsServerState<H>>) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if let Some(ref static_dir) = state.config.load().server.static_dir {
        if let Some(target) = resolve_safe_static_path(static_dir, "app.js") {
            if let Ok(content) = fs::read(&target) {
                let mut response = content.into_response();
                apply_security_headers(
                    response.headers_mut(),
                    "application/javascript; charset=utf-8",
                );
                return response;
            }
        }
    }

    let mut response = format!(
        "{}\n{}\n{}",
        crate::protocol::JAVASCRIPT_CONTRACT,
        include_str!("protocol.js"),
        APP_JS
    )
    .into_response();
    apply_security_headers(
        response.headers_mut(),
        "application/javascript; charset=utf-8",
    );
    response
}

/// SDK runtime cannot be replaced by a custom UI package.
pub async fn serve_sdk_types() -> Response {
    let mut response = include_str!("sdk/client.d.ts").into_response();
    apply_security_headers(response.headers_mut(), "text/plain; charset=utf-8");
    response
}

pub async fn serve_sdk() -> Response {
    let mut response = format!(
        "{}\n{}\n{}",
        crate::protocol::JAVASCRIPT_CONTRACT,
        include_str!("protocol.js"),
        include_str!("sdk/client.js")
    )
    .into_response();
    apply_security_headers(
        response.headers_mut(),
        "application/javascript; charset=utf-8",
    );
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-cache"));
    response
}

pub async fn serve_favicon<H>(State(state): State<WsServerState<H>>) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    if let Some(ref static_dir) = state.config.load().server.static_dir {
        if let Some(target) = resolve_safe_static_path(static_dir, "favicon.svg") {
            if let Ok(content) = fs::read(&target) {
                let mut response = content.into_response();
                apply_security_headers(response.headers_mut(), "image/svg+xml");
                return response;
            }
        }
    }

    let mut response = FAVICON_SVG.into_response();
    apply_security_headers(response.headers_mut(), "image/svg+xml");
    response
}

pub async fn serve_static_asset<H>(
    State(state): State<WsServerState<H>>,
    AxumPath(path): AxumPath<String>,
) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let clean = path.trim_start_matches('/');

    if let Some(ref static_dir) = state.config.load().server.static_dir {
        if let Some(safe_path) = resolve_safe_static_path(static_dir, clean) {
            if let Ok(bytes) = fs::read(&safe_path) {
                let mime = mime_for_path(&safe_path);
                let mut response = bytes.into_response();
                apply_security_headers(response.headers_mut(), mime);
                return response;
            }
        }
    }

    match clean {
        "style.css" => serve_style::<H>(State(state)).await,
        "app.js" => serve_app::<H>(State(state)).await,
        "accounts.js" => {
            let mut response = crate::ui::assets::ACCOUNTS_JS.into_response();
            apply_security_headers(
                response.headers_mut(),
                "application/javascript; charset=utf-8",
            );
            response
                .headers_mut()
                .insert("cache-control", HeaderValue::from_static("no-cache"));
            response
        }
        "favicon.svg" | "favicon.ico" => serve_favicon::<H>(State(state)).await,
        "index.html" => serve_index::<H>(State(state)).await,
        _ => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

pub async fn serve_fallback<H>(State(state): State<WsServerState<H>>, uri: Uri) -> Response
where
    H: WsHelperBackend + Clone + 'static,
{
    let path = uri.path();
    if path.starts_with("/api/") {
        let mut response = (StatusCode::NOT_FOUND, r#"{"error":"not_found"}"#).into_response();
        apply_security_headers(response.headers_mut(), "application/json; charset=utf-8");
        response
    } else {
        serve_index::<H>(State(state)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_mappings() {
        assert_eq!(
            mime_for_path(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            mime_for_path(Path::new("main.css")),
            "text/css; charset=utf-8"
        );
        assert_eq!(
            mime_for_path(Path::new("bundle.js")),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            mime_for_path(Path::new("data.json")),
            "application/json; charset=utf-8"
        );
        assert_eq!(mime_for_path(Path::new("logo.svg")), "image/svg+xml");
        assert_eq!(mime_for_path(Path::new("icon.png")), "image/png");
        assert_eq!(mime_for_path(Path::new("picture.jpg")), "image/jpeg");
        assert_eq!(mime_for_path(Path::new("video.mp4")), "video/mp4");
        assert_eq!(mime_for_path(Path::new("song.mp3")), "audio/mpeg");
        assert_eq!(mime_for_path(Path::new("doc.pdf")), "application/pdf");
        assert_eq!(mime_for_path(Path::new("favicon.ico")), "image/x-icon");
        assert_eq!(
            mime_for_path(Path::new("unknown.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn safe_path_resolution_blocks_traversal() {
        let temp = std::env::temp_dir().join(format!("tdrv-ui-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();
        let file_path = temp.join("test.txt");
        fs::write(&file_path, "hello ui").unwrap();

        assert!(resolve_safe_static_path(&temp, "test.txt").is_some());
        assert!(resolve_safe_static_path(&temp, "../etc/passwd").is_none());
        assert!(resolve_safe_static_path(&temp, "..\\windows").is_none());
        assert!(resolve_safe_static_path(&temp, "test\0.txt").is_none());
        assert!(resolve_safe_static_path(&temp, "non_existent.txt").is_none());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn embedded_assets_contain_expected_markers() {
        assert!(INDEX_HTML.contains(r#"meta name="status" content="ok""#));
        assert!(INDEX_HTML.contains("TealDrive"));
        assert!(INDEX_HTML.contains("lightboxModal"));
        assert!(INDEX_HTML.contains("settingsModal"));
        assert!(STYLE_CSS.contains("--teal-500"));
        assert!(STYLE_CSS.contains("modal-lightbox"));
        assert!(STYLE_CSS.contains("settings-body"));
        assert!(APP_JS.contains("TealDriveWire"));
        assert!(crate::protocol::JAVASCRIPT_CONTRACT.contains("ListDirectory"));
        assert!(APP_JS.contains("/api/preview"));
        assert!(APP_JS.contains("openLightbox"));
        assert!(APP_JS.contains("/api/admin/config"));
        assert!(FAVICON_SVG.contains("<svg"));
    }
}
