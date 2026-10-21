use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
use crate::error::AppError;
use crate::live_monitor::ActiveRequestRegistry;
use crate::provider::HttpUpstreamProvider;
use crate::state::AppState;

pub const MAX_REFERENCE_IMAGE_PAYLOAD_BYTES: usize = 10 * 1024 * 1024; // 10 MB decoded image limit
pub const MAX_REFERENCE_IMAGE_INPUT_CHARS: usize = 15 * 1024 * 1024; // ~15 MB raw input string limit

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageFormat {
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }

    pub fn matches_mime(&self, mime: &str) -> bool {
        let m = mime.to_ascii_lowercase();
        match self {
            Self::Png => m == "image/png",
            Self::Jpeg => m == "image/jpeg" || m == "image/jpg",
            Self::Webp => m == "image/webp",
            Self::Gif => m == "image/gif",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    pub fn from_magic_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 8 && bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        {
            Some(Self::Png)
        } else if bytes.len() >= 3 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else if bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
            Some(Self::Gif)
        } else {
            None
        }
    }
}

pub fn is_recognized_image_magic(bytes: &[u8]) -> bool {
    if ImageFormat::from_magic_bytes(bytes).is_some() {
        return true;
    }
    // AVIF / HEIF: bytes 4..8 == b"ftyp" and brand like avif, avis, mif1, heic
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"avif"
            || brand == b"avis"
            || brand == b"mif1"
            || brand == b"msf1"
            || brand == b"heic"
        {
            return true;
        }
    }
    // SVG: begins with <svg or <?xml (possibly with leading whitespace/BOM)
    let trimmed = match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start(),
        Err(_) => {
            let valid_len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            std::str::from_utf8(&bytes[..valid_len])
                .map(str::trim_start)
                .unwrap_or("")
        }
    };
    if trimmed.starts_with("<svg") || trimmed.starts_with("<?xml") {
        return true;
    }
    // BMP
    if bytes.len() >= 2 && &bytes[0..2] == b"BM" {
        return true;
    }
    // TIFF
    if bytes.len() >= 4 && (&bytes[0..4] == b"II*\0" || &bytes[0..4] == b"MM\0*") {
        return true;
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedReferenceImage {
    pub format: ImageFormat,
    pub data: Vec<u8>,
}

pub fn validate_reference_image(raw: &str) -> Result<ValidatedReferenceImage, AppError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "reference_image cannot be empty".to_string(),
        ));
    }

    // Predecode size check to avoid base64 allocation/memory DoS
    if raw.len() > MAX_REFERENCE_IMAGE_INPUT_CHARS {
        return Err(AppError::BadRequest(format!(
            "reference_image payload exceeds maximum allowed size of {MAX_REFERENCE_IMAGE_INPUT_CHARS} characters"
        )));
    }

    // Explicitly reject unambiguous host file paths and file URIs
    if raw.starts_with("file:")
        || raw.starts_with('~')
        || raw.starts_with("./")
        || raw.starts_with("../")
        || raw.contains('\\')
    {
        return Err(AppError::BadRequest(
            "reference_image cannot be an arbitrary host file path; use a base64 data URL or base64 image".to_string(),
        ));
    }

    if raw.starts_with("data:") {
        parse_data_url(raw)
    } else {
        let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        let decoded = match base64::engine::general_purpose::STANDARD.decode(&cleaned) {
            Ok(bytes) => bytes,
            Err(_) => {
                if raw.starts_with('/') || raw.contains('.') || raw.contains('/') {
                    return Err(AppError::BadRequest(
                        "reference_image cannot be an arbitrary host file path; use a base64 data URL or base64 image".to_string(),
                    ));
                }
                return Err(AppError::BadRequest(
                    "reference_image must be a valid base64 data URL or base64 image string"
                        .to_string(),
                ));
            }
        };

        if decoded.len() > MAX_REFERENCE_IMAGE_PAYLOAD_BYTES {
            return Err(AppError::BadRequest(format!(
                "reference_image decoded bytes exceed limit of {MAX_REFERENCE_IMAGE_PAYLOAD_BYTES} bytes"
            )));
        }

        let format = match ImageFormat::from_magic_bytes(&decoded) {
            Some(fmt) => fmt,
            None => {
                if raw.starts_with('/') || raw.contains('.') || raw.contains('/') {
                    return Err(AppError::BadRequest(
                        "reference_image cannot be an arbitrary host file path; use a base64 data URL or base64 image".to_string(),
                    ));
                }
                return Err(AppError::BadRequest(
                    "reference_image does not contain recognized image magic bytes (PNG, JPEG, WebP, GIF)".to_string(),
                ));
            }
        };

        Ok(ValidatedReferenceImage {
            format,
            data: decoded,
        })
    }
}

fn parse_data_url(raw: &str) -> Result<ValidatedReferenceImage, AppError> {
    let rest = raw
        .strip_prefix("data:")
        .ok_or_else(|| AppError::BadRequest("invalid data URL".to_string()))?;
    let (meta, b64) = rest.split_once(";base64,").ok_or_else(|| {
        AppError::BadRequest(
            "reference_image data URL must use base64 encoding (expected ;base64,)".to_string(),
        )
    })?;

    let mime = meta
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if !mime.starts_with("image/") {
        return Err(AppError::BadRequest(format!(
            "reference_image data URL must have an image MIME type, found: {meta}"
        )));
    }

    // Predecode size check on the base64 portion
    if b64.len() > MAX_REFERENCE_IMAGE_INPUT_CHARS {
        return Err(AppError::BadRequest(format!(
            "reference_image base64 payload exceeds maximum allowed size of {MAX_REFERENCE_IMAGE_INPUT_CHARS} characters"
        )));
    }

    let cleaned: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .map_err(|e| {
            AppError::BadRequest(format!("invalid base64 payload in reference_image: {e}"))
        })?;
    if decoded.len() > MAX_REFERENCE_IMAGE_PAYLOAD_BYTES {
        return Err(AppError::BadRequest(format!(
            "reference_image decoded bytes exceed limit of {MAX_REFERENCE_IMAGE_PAYLOAD_BYTES} bytes"
        )));
    }
    let format = ImageFormat::from_magic_bytes(&decoded).ok_or_else(|| {
        AppError::BadRequest(
            "reference_image data URL payload does not match valid image magic bytes".to_string(),
        )
    })?;

    // Handle magic MIME mismatch
    if !format.matches_mime(&mime) {
        return Err(AppError::BadRequest(format!(
            "reference_image declared MIME type '{mime}' does not match detected format '{}'",
            format.mime_type()
        )));
    }

    Ok(ValidatedReferenceImage {
        format,
        data: decoded,
    })
}

pub fn validate_external_reference_image(raw: &str) -> Result<(), AppError> {
    // 1. Explicitly reject clear host file paths and file URIs
    if raw.starts_with("file:")
        || raw.starts_with('~')
        || raw.starts_with("./")
        || raw.starts_with("../")
        || raw.contains('\\')
    {
        return Err(AppError::BadRequest(
            "reference_image cannot be an arbitrary host file path; use a base64 data URL, base64 image, or web URL".to_string(),
        ));
    }

    // 2. Web URL: allow http/https without decoding
    if raw.starts_with("http://") || raw.starts_with("https://") {
        if let Ok(url) = reqwest::Url::parse(raw) {
            if (url.scheme() == "http" || url.scheme() == "https") && url.has_host() {
                return Ok(());
            }
        }
        return Err(AppError::BadRequest(
            "reference_image URL is invalid; expected valid http or https URL".to_string(),
        ));
    }

    // 3. Data URL: validate image MIME type and base64 marker without decoding payload body
    // This preserves provider-specific formats like AVIF/SVG and avoids unnecessary memory allocation
    if raw.starts_with("data:") {
        let rest = raw
            .strip_prefix("data:")
            .ok_or_else(|| AppError::BadRequest("invalid data URL".to_string()))?;
        let (meta, b64) = rest.split_once(";base64,").ok_or_else(|| {
            AppError::BadRequest(
                "reference_image data URL must use base64 encoding (expected ;base64,)".to_string(),
            )
        })?;
        let mime = meta
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !mime.starts_with("image/") {
            return Err(AppError::BadRequest(format!(
                "reference_image data URL must have an image MIME type, found: {meta}"
            )));
        }
        if b64.trim().is_empty() {
            return Err(AppError::BadRequest(
                "reference_image data URL base64 payload cannot be empty".to_string(),
            ));
        }
        return Ok(());
    }

    if raw.split_once(':').is_some() {
        return Err(AppError::BadRequest(
            "unsupported reference_image URL scheme".to_string(),
        ));
    }

    // 4. Raw base64 vs Host Path:
    // Decode only the first small prefix (up to 128 chars = <= 96 bytes) to verify image magic
    // without allocating the entire payload in memory.
    let mut prefix_chars = Vec::with_capacity(128);
    for c in raw.chars() {
        if !c.is_whitespace() {
            prefix_chars.push(c);
            if prefix_chars.len() == 128 {
                break;
            }
        }
    }
    if prefix_chars.len() < 4 {
        return Err(AppError::BadRequest(
            "reference_image string is too short to be a valid base64 image".to_string(),
        ));
    }
    let chunk_len = (prefix_chars.len() / 4) * 4;
    let prefix_str: String = prefix_chars[..chunk_len].iter().collect();
    let decoded_prefix = match base64::engine::general_purpose::STANDARD.decode(&prefix_str) {
        Ok(bytes) => bytes,
        Err(_) => {
            if raw.starts_with('/') || raw.contains('.') || raw.contains('/') {
                return Err(AppError::BadRequest(
                    "reference_image cannot be an arbitrary host file path; use a base64 data URL, base64 image, or web URL".to_string(),
                ));
            }
            return Err(AppError::BadRequest(
                "reference_image must be a valid base64 data URL or base64 image string"
                    .to_string(),
            ));
        }
    };

    if !is_recognized_image_magic(&decoded_prefix) {
        if raw.starts_with('/') || raw.contains('.') || raw.contains('/') {
            return Err(AppError::BadRequest(
                "reference_image cannot be an arbitrary host file path; use a base64 data URL, base64 image, or web URL".to_string(),
            ));
        }
        return Err(AppError::BadRequest(
            "reference_image does not contain recognized image magic bytes".to_string(),
        ));
    }

    Ok(())
}

pub struct ImageCancellationGuard {
    pub live_registry: Arc<ActiveRequestRegistry>,
    pub request_id: String,
    pub completed: bool,
}

impl Drop for ImageCancellationGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.live_registry.finish(&self.request_id, "cancelled");
        }
    }
}

pub struct RunDirGuard {
    path: std::path::PathBuf,
    active: bool,
}

impl RunDirGuard {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path, active: true }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for RunDirGuard {
    fn drop(&mut self) {
        if self.active {
            let auth_file = self.path.join("auth.json");
            if auth_file.exists() {
                let _ = std::fs::write(&auth_file, b"");
                let _ = std::fs::remove_file(&auth_file);
            }
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub fn is_codex_image_execution_safe() -> bool {
    // Codex CLI executes arbitrary shell commands through its agent runtime.
    // Without an isolated OS sandbox demonstrably restricting tools strictly to
    // image_gen only (with no shell/process execution or credential exfiltration),
    // running Codex CLI with client-controlled prompts is fundamentally unsafe.
    false
}

pub fn build_codex_prompt(prompt: &str, out_file: &std::path::Path, has_reference: bool) -> String {
    let out_str = out_file.to_string_lossy();
    if has_reference {
        format!(
            "Using the reference image (-i) as visual context, use the image_gen tool to generate an image adhering to: {prompt}. Save the resulting image to {out_str}."
        )
    } else {
        format!(
            "Use the image_gen tool to generate an image adhering to: {prompt}. Save the resulting image to {out_str}."
        )
    }
}

pub async fn verify_image_file(
    path: &std::path::Path,
) -> Option<(ImageFormat, std::time::SystemTime)> {
    let sym_meta = tokio::fs::symlink_metadata(path).await.ok()?;
    if sym_meta.file_type().is_symlink() || !sym_meta.is_file() || sym_meta.len() < 3 {
        return None;
    }
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await.ok()?;
    let mut header = [0u8; 16];
    let n = file.read(&mut header).await.ok()?;
    let format = ImageFormat::from_magic_bytes(&header[..n])?;
    let mtime = sym_meta
        .modified()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    Some((format, mtime))
}

pub async fn find_generated_image(
    run_dir: &std::path::Path,
    designated_out: &std::path::Path,
) -> Option<std::path::PathBuf> {
    // 1. Designated output file precedence
    if verify_image_file(designated_out).await.is_some() {
        return Some(designated_out.to_path_buf());
    }

    let parent = designated_out.parent().unwrap_or(run_dir);
    for ext in &["png", "jpg", "jpeg", "webp", "gif"] {
        let alt = parent.join(format!("output.{ext}"));
        if alt != designated_out && verify_image_file(&alt).await.is_some() {
            return Some(alt);
        }
    }

    // 2. generated_images subdirectory precedence
    let gen_dir = run_dir.join("generated_images");
    if gen_dir.is_dir() {
        if let Some(p) = find_best_image_in_dir(&gen_dir, false).await {
            return Some(p);
        }
    }

    // 3. Root run_dir scan
    find_best_image_in_dir(run_dir, true).await
}

async fn find_best_image_in_dir(
    dir: &std::path::Path,
    recursive: bool,
) -> Option<std::path::PathBuf> {
    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    let mut candidates: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        // Explicitly ignore symlinks to prevent traversal and leakage
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_type.is_dir() {
            if !recursive {
                continue;
            }
            if name == "skills"
                || name == "plugins"
                || name == "cache"
                || name == "tmp"
                || name == "generated_images"
                || name.starts_with('.')
            {
                continue;
            }
            if let Some(nested) = Box::pin(find_best_image_in_dir(&path, true)).await {
                if let Some((_, mtime)) = verify_image_file(&nested).await {
                    candidates.push((nested, mtime));
                }
            }
        } else if file_type.is_file() {
            if name == "auth.json"
                || name == "config.toml"
                || name.starts_with("ref_input")
                || name.starts_with('.')
            {
                continue;
            }
            if let Some((_, mtime)) = verify_image_file(&path).await {
                candidates.push((path, mtime));
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.0.to_string_lossy().cmp(&b.0.to_string_lossy()))
    });

    candidates.into_iter().next().map(|(p, _)| p)
}

pub async fn image_generations(
    auth: AuthenticatedUser,
    State(state): State<AppState>,
    payload_result: Result<Json<Value>, JsonRejection>,
) -> Result<Response, AppError> {
    let start_time = Instant::now();
    let timestamp = chrono::Utc::now().timestamp() as f64;
    let request_id = uuid::Uuid::new_v4().to_string();
    let client_label = if !auth.name.trim().is_empty() {
        auth.name.clone()
    } else {
        crate::db::mask_api_key(&auth.key)
    };

    // Register image request from route entry immediately
    state
        .live_registry
        .register(request_id.clone(), client_label, "image".to_string(), false);

    let mut cancel_guard = ImageCancellationGuard {
        live_registry: state.live_registry.clone(),
        request_id: request_id.clone(),
        completed: false,
    };

    let result = image_generations_inner(
        &state,
        &auth,
        payload_result,
        &request_id,
        timestamp,
        start_time,
    )
    .await;

    cancel_guard.completed = true;
    match &result {
        Ok(_) => {
            state.live_registry.finish(&request_id, "success");
        }
        Err(_) => {
            state.live_registry.finish(&request_id, "error");
        }
    }
    result
}

async fn image_generations_inner(
    state: &AppState,
    auth: &AuthenticatedUser,
    payload_result: Result<Json<Value>, JsonRejection>,
    request_id: &str,
    timestamp: f64,
    start_time: Instant,
) -> Result<Response, AppError> {
    let Json(mut payload) = payload_result.map_err(|err| AppError::BadRequest(err.to_string()))?;
    let object = payload
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("request body must be a JSON object".to_string()))?;

    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("prompt must be a non-empty string".to_string()))?
        .to_string();
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("model must be a non-empty string".to_string()))?
        .to_string();

    let raw_reference_image = {
        let img_val = object.get("image");
        let ref_val = object.get("reference_image");

        for (field_name, val) in [("image", img_val), ("reference_image", ref_val)] {
            if let Some(v) = val {
                match v.as_str() {
                    Some(s) => {
                        if s.trim().is_empty() {
                            return Err(AppError::BadRequest(format!(
                                "{field_name} cannot be empty"
                            )));
                        }
                    }
                    None => {
                        return Err(AppError::BadRequest(format!(
                            "{field_name} must be a non-empty string"
                        )));
                    }
                }
            }
        }

        img_val.or(ref_val).and_then(Value::as_str).map(str::trim)
    };

    if let Some(raw) = raw_reference_image {
        if raw.len() > MAX_REFERENCE_IMAGE_INPUT_CHARS {
            return Err(AppError::BadRequest(format!(
                "reference_image payload exceeds maximum allowed size of {MAX_REFERENCE_IMAGE_INPUT_CHARS} characters"
            )));
        }
    }

    let is_mock = state.config.use_mock_provider;
    let is_codex = is_codex_image_model(&model);

    if is_mock {
        let _ = if let Some(raw) = raw_reference_image {
            if raw.starts_with("http://") || raw.starts_with("https://") {
                if let Ok(url) = reqwest::Url::parse(raw) {
                    if (url.scheme() == "http" || url.scheme() == "https") && url.has_host() {
                        None
                    } else {
                        return Err(AppError::BadRequest(
                            "reference_image URL is invalid".to_string(),
                        ));
                    }
                } else {
                    return Err(AppError::BadRequest(
                        "reference_image URL is invalid".to_string(),
                    ));
                }
            } else {
                Some(validate_reference_image(raw)?)
            }
        } else {
            None
        };

        state
            .live_registry
            .update_routing(request_id, "MockProvider", &model, "mock");

        let response = (
            StatusCode::OK,
            Json(mock_image_response(&model, &prompt, object)),
        )
            .into_response();
        let _ = state.db.increment_total_requests(&auth.key);
        let _ = state.db.record_request(
            Some("MockProvider"),
            &model,
            timestamp,
            "ok",
            0,
            0,
            start_time.elapsed().as_secs_f64() * 1000.0,
            None,
        );
        return Ok(response);
    }

    if is_codex {
        let validated_reference = if let Some(raw) = raw_reference_image {
            if raw.starts_with("http://") || raw.starts_with("https://") {
                if let Ok(url) = reqwest::Url::parse(raw) {
                    if (url.scheme() == "http" || url.scheme() == "https") && url.has_host() {
                        None
                    } else {
                        return Err(AppError::BadRequest(
                            "reference_image URL is invalid".to_string(),
                        ));
                    }
                } else {
                    return Err(AppError::BadRequest(
                        "reference_image URL is invalid".to_string(),
                    ));
                }
            } else {
                Some(validate_reference_image(raw)?)
            }
        } else {
            None
        };

        return generate_codex_image(
            state,
            auth,
            &model,
            &prompt,
            validated_reference.as_ref(),
            request_id,
            timestamp,
            start_time,
        )
        .await;
    }

    // External provider passthrough:
    // Reject clear host paths/file URIs, preserve AVIF/SVG without decoding, avoid unnecessary allocation
    if let Some(raw) = raw_reference_image {
        validate_external_reference_image(raw)?;
    }

    let (provider, routing_prefix) = resolve_provider(state, &model)?;
    let account_label = format!("provider:{}", provider.prefix);
    let masked_account = crate::live_monitor::mask_account_email(&account_label);

    state
        .live_registry
        .update_routing(request_id, &provider.name, &model, &masked_account);

    let upstream = Arc::new(HttpUpstreamProvider::new(
        Some(provider.base_url),
        Some(provider.api_key),
        state.config.upstream_connect_timeout_secs,
        state.config.upstream_read_timeout_secs,
        state.config.upstream_request_timeout_secs,
    ));

    match upstream
        .generate_image(&payload, routing_prefix.as_deref())
        .await
    {
        Ok(response) => {
            let _ = state.db.increment_total_requests(&auth.key);
            let _ = state.db.record_request(
                Some(&account_label),
                &model,
                timestamp,
                "ok",
                0,
                0,
                start_time.elapsed().as_secs_f64() * 1000.0,
                None,
            );
            Ok((StatusCode::OK, Json(response)).into_response())
        }
        Err(err) => {
            let err_string = err.to_string();
            let _ = state.db.record_request(
                Some(&account_label),
                &model,
                timestamp,
                "error",
                0,
                0,
                start_time.elapsed().as_secs_f64() * 1000.0,
                Some(&err_string),
            );
            Err(err)
        }
    }
}

fn resolve_provider(
    state: &AppState,
    model: &str,
) -> Result<(crate::db::ProviderRecord, Option<String>), AppError> {
    if let Some(prefix) = model.split_once('/').map(|(prefix, _)| prefix) {
        if let Some(provider) = state
            .db
            .get_provider_by_prefix(prefix)
            .ok()
            .flatten()
            .filter(|provider| provider.is_active)
        {
            return Ok((provider, Some(prefix.to_string())));
        }
    }

    let provider = state.db.list_providers().ok().and_then(|providers| {
        providers.into_iter().find(|provider| {
            provider.is_active
                && provider.models.as_array().is_some_and(|models| {
                    models.iter().any(|candidate| {
                        candidate.as_str() == Some(model)
                            || candidate.get("id").and_then(Value::as_str) == Some(model)
                    })
                })
        })
    });

    provider.map(|provider| (provider, None)).ok_or_else(|| {
        AppError::NotFound(format!(
            "Model '{model}' not found or no active provider configured"
        ))
    })
}

fn mock_image_response(
    model: &str,
    prompt: &str,
    payload: &serde_json::Map<String, Value>,
) -> Value {
    let n = payload
        .get("n")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 10);
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in model.bytes().chain([0]).chain(prompt.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let data = (0..n)
        .map(|index| {
            json!({
                "url": format!("mock://images/{hash:016x}-{index}.png"),
                "revised_prompt": prompt,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "created": 0,
        "data": data,
    })
}

pub fn is_codex_image_model(model: &str) -> bool {
    let m = model.to_lowercase();
    m == "cx/image-gen"
        || m == "image-gen"
        || m == "cx/gpt-image"
        || m == "cx/gpt-image-2"
        || m == "gpt-image-2"
        || m == "codex-image"
        || m.starts_with("cx/image")
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_codex_image(
    state: &AppState,
    auth: &AuthenticatedUser,
    model: &str,
    prompt: &str,
    reference_image: Option<&ValidatedReferenceImage>,
    request_id: &str,
    timestamp: f64,
    start_time: Instant,
) -> Result<Response, AppError> {
    if !is_codex_image_execution_safe() {
        let masked_account = "codex:disabled";
        state
            .live_registry
            .update_routing(request_id, "Codex", model, masked_account);

        let err_msg = "Codex image generation is disabled: safe execution requires an isolated OS sandbox restricting tools to image_gen only, which is not available in this environment";
        let _ = state.db.record_request(
            Some(masked_account),
            model,
            timestamp,
            "error",
            0,
            0,
            start_time.elapsed().as_secs_f64() * 1000.0,
            Some(err_msg),
        );

        return Err(AppError::ServiceUnavailable(err_msg.to_string()));
    }

    let mut lease = match state.codex_pool.acquire_lease_with_pacing().await {
        Some(l) => l,
        None => {
            let (status, msg) = state.codex_pool.diagnose_unavailable();
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::RateLimit(msg));
            } else {
                return Err(AppError::BadGateway(msg));
            }
        }
    };

    let acc = lease.account.clone();
    let email_label = acc.email.read().unwrap().clone();
    let account_label = format!("codex:{}", email_label);
    let masked_account = crate::live_monitor::mask_account_email(&email_label);

    state
        .live_registry
        .update_routing(request_id, "Codex", model, &masked_account);

    let _gen_guard = acc.generation_lock.lock().await;

    if let Err(e) = state.codex_pool.ensure_token(&acc).await {
        lease.commit_error(&e, false);
        return Err(AppError::BadGateway(format!("Token refresh failed: {e}")));
    }

    let scratch_base =
        std::env::var("TMPDIR").unwrap_or_else(|_| "/home/namhv/.hermes/cache/scratch".to_string());
    let run_id = uuid::Uuid::new_v4().simple();
    let run_dir = std::path::PathBuf::from(&scratch_base).join(format!("codex-img-{run_id}"));
    if let Err(e) = tokio::fs::create_dir_all(&run_dir).await {
        lease.release();
        return Err(AppError::Internal(format!(
            "Failed to create run directory: {e}"
        )));
    }

    let run_guard = RunDirGuard::new(run_dir.clone());

    if let Err(e) = tokio::fs::copy(&acc.auth_path, run_dir.join("auth.json")).await {
        lease.release();
        return Err(AppError::Internal(format!(
            "Failed to copy credentials: {e}"
        )));
    }

    let out_file = run_dir.join("output.png");
    let codex_prompt = build_codex_prompt(prompt, &out_file, reference_image.is_some());

    let mut cmd = tokio::process::Command::new("codex");
    cmd.arg("exec");
    if let Some(ref_img) = reference_image {
        let ref_path = run_dir.join(format!("ref_input.{}", ref_img.format.extension()));
        if let Err(e) = tokio::fs::write(&ref_path, &ref_img.data).await {
            lease.release();
            return Err(AppError::Internal(format!(
                "Failed to write reference image: {e}"
            )));
        }
        cmd.arg("-i").arg(&ref_path);
    }
    cmd.args(["--skip-git-repo-check", &codex_prompt]);
    cmd.env("CODEX_HOME", &run_dir);
    cmd.current_dir(&run_dir);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let timeout_dur = std::time::Duration::from_secs(240);
    let child_res = tokio::time::timeout(timeout_dur, cmd.output()).await;

    let output = match child_res {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            let err_msg = format!("Failed to spawn codex CLI: {e}");
            lease.commit_error(&err_msg, false);
            return Err(AppError::BadGateway(err_msg));
        }
        Err(_) => {
            let err_msg = "Codex image generation timed out after 240s".to_string();
            lease.commit_error_with_cooldown(&err_msg, 30.0);
            return Err(AppError::BadGateway(err_msg));
        }
    };

    let resolved_img_path = find_generated_image(&run_dir, &out_file).await;

    let img_path = match resolved_img_path {
        Some(p) => p,
        None => {
            let stdout_snippet = String::from_utf8_lossy(&output.stdout);
            let stderr_snippet = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout_snippet}\n{stderr_snippet}");
            let masked = crate::quota_refresh::sanitize_error_message(&combined);
            lease.commit_error(&masked, false);
            return Err(AppError::BadGateway(format!(
                "Codex did not produce an image. Exit code: {}. Output: {}",
                output.status.code().unwrap_or(-1),
                masked.chars().take(300).collect::<String>()
            )));
        }
    };

    let image_bytes = match tokio::fs::read(&img_path).await {
        Ok(b) if !b.is_empty() => b,
        _ => {
            lease.commit_error("Generated image file is empty", false);
            return Err(AppError::BadGateway(
                "Generated image file is empty".to_string(),
            ));
        }
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);
    let data_url = format!("data:image/png;base64,{b64}");

    drop(run_guard);

    lease.commit_success();

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    let _ = state.db.increment_total_requests(&auth.key);
    let _ = state.db.record_request(
        Some(&account_label),
        model,
        timestamp,
        "ok",
        prompt.len() as i64 / 4,
        0,
        duration_ms,
        None,
    );

    let response_body = serde_json::json!({
        "created": timestamp as i64,
        "data": [
            {
                "b64_json": b64,
                "url": data_url,
                "revised_prompt": prompt
            }
        ]
    });

    Ok((StatusCode::OK, Json(response_body)).into_response())
}
