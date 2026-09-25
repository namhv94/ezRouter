use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::{Path as StdPath, PathBuf};

pub fn get_ui_dist_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AG_UI_DIST_DIR") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return p;
        }
    }
    let p = PathBuf::from("ui/dist");
    if p.exists() {
        return p;
    }
    let manifest_p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui/dist");
    if manifest_p.exists() {
        return manifest_p;
    }
    p
}

pub fn resolve_safe_static_path(base_dir: &StdPath, request_path: &str) -> Option<PathBuf> {
    let cleaned = request_path.trim_start_matches('/');
    if cleaned.is_empty() {
        return None;
    }

    let rel_path = StdPath::new(cleaned);
    for component in rel_path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => return None,
        }
    }

    let candidate = base_dir.join(rel_path);
    if let (Ok(canonical_base), Ok(canonical_target)) =
        (base_dir.canonicalize(), candidate.canonicalize())
    {
        if canonical_target.starts_with(&canonical_base) && canonical_target.is_file() {
            return Some(canonical_target);
        }
    }
    None
}

pub fn get_mime_type(path: &StdPath) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

pub async fn serve_root() -> Response {
    let dist_dir = get_ui_dist_dir();
    let index_file = dist_dir.join("index.html");

    if index_file.exists() && index_file.is_file() {
        match tokio::fs::read(&index_file).await {
            Ok(bytes) => (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                bytes,
            )
                .into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Error reading index.html",
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "ag-proxy-rust staging service",
        )
            .into_response()
    }
}

pub async fn serve_assets(Path(path): Path<String>) -> Response {
    let dist_dir = get_ui_dist_dir();
    let assets_dir = dist_dir.join("assets");

    if let Some(target_file) = resolve_safe_static_path(&assets_dir, &path) {
        let mime = get_mime_type(&target_file);
        match tokio::fs::read(&target_file).await {
            Ok(bytes) => (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                bytes,
            )
                .into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

pub async fn serve_favicon() -> Response {
    let dist_dir = get_ui_dist_dir();
    let favicon_file = dist_dir.join("favicon.ico");

    if favicon_file.exists() && favicon_file.is_file() {
        match tokio::fs::read(&favicon_file).await {
            Ok(bytes) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "image/x-icon")],
                bytes,
            )
                .into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_resolve_safe_static_path_prevents_traversal() {
        let tmp = std::env::temp_dir().join(format!("ag_proxy_ui_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(tmp.join("assets"));
        let mut f = File::create(tmp.join("assets/test.js")).unwrap();
        writeln!(f, "console.log('test');").unwrap();

        // 1. Valid path inside assets
        let found = resolve_safe_static_path(&tmp.join("assets"), "test.js");
        assert!(found.is_some());

        // 2. Traversal attempt with ../
        let bad1 = resolve_safe_static_path(&tmp.join("assets"), "../Cargo.toml");
        assert!(bad1.is_none());

        // 3. Traversal attempt with leading /
        let bad2 = resolve_safe_static_path(&tmp.join("assets"), "/../etc/passwd");
        assert!(bad2.is_none());

        // 4. Traversal attempt nested
        let bad3 = resolve_safe_static_path(&tmp.join("assets"), "sub/../../etc/passwd");
        assert!(bad3.is_none());

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn test_get_mime_type() {
        assert_eq!(
            get_mime_type(StdPath::new("file.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            get_mime_type(StdPath::new("file.js")),
            "application/javascript; charset=utf-8"
        );
        assert_eq!(
            get_mime_type(StdPath::new("file.css")),
            "text/css; charset=utf-8"
        );
        assert_eq!(get_mime_type(StdPath::new("file.svg")), "image/svg+xml");
        assert_eq!(get_mime_type(StdPath::new("file.ico")), "image/x-icon");
        assert_eq!(
            get_mime_type(StdPath::new("file.unknown")),
            "application/octet-stream"
        );
    }
}
