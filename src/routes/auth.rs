use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::{account, state::AppState};

#[derive(Debug, Deserialize)]
pub struct AuthLoginQuery {
    pub origin: Option<String>,
    pub direct: Option<String>,
    pub redirect_uri: Option<String>,
}

pub fn is_allowed_origin(origin: &str, redirect_uri_config: &str) -> bool {
    let trimmed = origin.trim();
    if trimmed.is_empty() {
        return false;
    }
    let parsed = match reqwest::Url::parse(trimmed) {
        Ok(u) => u,
        Err(_) => return false,
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    let host = match parsed.host_str() {
        Some(h) => h.to_lowercase(),
        None => return false,
    };

    if host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.ends_with(".localhost")
        || host == "example.com"
        || host.ends_with(".example.com")
    {
        return true;
    }
    if let Ok(allowed_env) = std::env::var("EZ_ALLOWED_DOMAINS") {
        for dom in allowed_env.split(',') {
            let d = dom.trim().to_lowercase();
            if !d.is_empty() && (host == d || host.ends_with(&format!(".{d}"))) {
                return true;
            }
        }
    }
    if host == "namhv.vip" || host.ends_with(".namhv.vip") {
        return true;
    }
    if let Ok(redir_parsed) = reqwest::Url::parse(redirect_uri_config) {
        if let Some(redir_host) = redir_parsed.host_str() {
            if host == redir_host.to_lowercase() {
                return true;
            }
        }
    }
    false
}

pub async fn auth_login(
    headers: axum::http::HeaderMap,
    Query(params): Query<AuthLoginQuery>,
    State(state): State<AppState>,
) -> Response {
    let redirect_uri = params
        .redirect_uri
        .as_deref()
        .unwrap_or(&state.config.oauth_redirect_uri);

    let safe_origin = params
        .origin
        .clone()
        .filter(|o| is_allowed_origin(o, &state.config.oauth_redirect_uri));

    let detected_origin = safe_origin.or_else(|| {
        let host = headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))
            .and_then(|h| h.to_str().ok())?;
        let proto = headers
            .get("x-forwarded-proto")
            .and_then(|p| p.to_str().ok())
            .unwrap_or("https");
        let candidate = format!("{proto}://{host}");
        if is_allowed_origin(&candidate, &state.config.oauth_redirect_uri) {
            Some(candidate)
        } else {
            None
        }
    });

    let start_res = state
        .account_pool
        .start_oauth(redirect_uri, detected_origin.as_deref());

    let auth_url = start_res.authorize_url;

    if params.direct.as_deref() == Some("1") || params.direct.as_deref() == Some("true") {
        return Redirect::to(&auth_url).into_response();
    }

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>Đăng nhập Google · ezRouter</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Be+Vietnam+Pro:wght@400;500;600;700&display=swap" rel="stylesheet">
  <style>
    :root {{ --bg: #080b10; --card: #0f1520; --line: #202836; --text: #f3f4f6; --muted: #94a3b8; --accent: #6366f1; }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: 'Be Vietnam Pro', system-ui, sans-serif;
      min-height: 100vh;
      display: grid;
      place-items: center;
      padding: 24px;
      background: radial-gradient(circle at 50% 15%, #6366f11c, transparent 65%), var(--bg);
      color: var(--text);
      -webkit-font-smoothing: antialiased;
    }}
    .auth-card {{
      width: min(440px, 100%);
      padding: 36px 30px;
      border: 1px solid var(--line);
      border-radius: 18px;
      background: var(--card);
      box-shadow: 0 25px 60px rgba(0,0,0,0.6);
      text-align: center;
    }}
    .brand-badge {{
      display: inline-flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 22px;
    }}
    .brand-logo {{
      width: 36px;
      height: 36px;
      display: grid;
      place-items: center;
      border-radius: 10px;
      background: linear-gradient(145deg, #696cf5, #8b5cf6);
      color: #fff;
      font-weight: 700;
      font-size: 15px;
      letter-spacing: -0.05em;
      box-shadow: 0 6px 18px rgba(99,102,241,0.35);
    }}
    .brand-name {{
      font-weight: 700;
      font-size: 16px;
      letter-spacing: -0.03em;
    }}
    h1 {{
      font-size: 21px;
      font-weight: 700;
      letter-spacing: -0.025em;
      margin-bottom: 10px;
      color: #fff;
    }}
    p.desc {{
      font-size: 12px;
      line-height: 1.65;
      color: var(--muted);
      margin-bottom: 28px;
    }}
    .google-btn {{
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 12px;
      width: 100%;
      min-height: 48px;
      padding: 0 20px;
      border-radius: 11px;
      border: 1px solid #ffffff1a;
      background: #ffffff;
      color: #1f2937;
      font-size: 13px;
      font-weight: 600;
      text-decoration: none;
      transition: transform 0.15s, box-shadow 0.15s, background 0.15s;
      box-shadow: 0 4px 14px rgba(0,0,0,0.25);
    }}
    .google-btn:hover {{
      background: #f8fafc;
      transform: translateY(-1px);
      box-shadow: 0 6px 20px rgba(0,0,0,0.35);
    }}
    .google-btn:active {{
      transform: translateY(0);
    }}
    .google-icon {{
      width: 18px;
      height: 18px;
      flex-shrink: 0;
    }}
    .hint {{
      margin-top: 22px;
      font-size: 11px;
      color: #64748b;
      line-height: 1.6;
    }}
  </style>
</head>
<body>
  <div class="auth-card">
    <div class="brand-badge">
      <div class="brand-logo">ez</div>
      <div class="brand-name">ezRouter</div>
    </div>
    <h1>Thêm Google Account</h1>
    <p class="desc">Cấp quyền OAuth Cloud Code AI an toàn để đưa tài khoản vào hệ thống xoay vòng thông minh.</p>
    <a class="google-btn" href="{auth_url}">
      <svg class="google-icon" viewBox="0 0 24 24">
        <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
        <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
        <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22.81-.63z"/>
        <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84c.87-2.6 3.3-4.52 6.16-4.52z"/>
      </svg>
      Đăng nhập với Google
    </a>
    <p class="hint">Sau khi đăng nhập, hệ thống sẽ tự động hoàn tất và đồng bộ về trang quản trị.</p>
  </div>
</body>
</html>"##
    );

    Html(html).into_response()
}

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
    pub state: Option<String>,
}

pub async fn auth_callback(
    Query(params): Query<AuthCallbackQuery>,
    State(state): State<AppState>,
) -> Response {
    let mut target_origin = String::new();
    if let Some(ref st) = params.state {
        if let Some((orig, _)) = st.split_once('|') {
            let candidate = orig.trim().trim_end_matches('/');
            if is_allowed_origin(candidate, &state.config.oauth_redirect_uri) {
                target_origin = candidate.to_string();
            }
        }
    }

    if let Some(err) = params.error {
        let err_esc = html_escape(&err);
        let html = format!(
            r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>Lỗi xác thực · ezRouter</title>
  <style>
    body {{ font-family: system-ui, sans-serif; background: #080b10; color: #f3f4f6; min-height: 100vh; display: grid; place-items: center; padding: 20px; }}
    .card {{ max-width: 420px; width: 100%; padding: 32px; border: 1px solid #f8717130; border-radius: 16px; background: #111622; text-align: center; }}
    h1 {{ color: #f87171; font-size: 20px; margin-bottom: 12px; }}
    p {{ color: #94a3b8; font-size: 13px; line-height: 1.6; margin-bottom: 20px; }}
    a {{ color: #818cf8; text-decoration: none; font-size: 13px; font-weight: 600; }}
  </style>
</head>
<body>
  <div class="card">
    <h1>❌ Lỗi kết nối Google</h1>
    <p>{err_esc}</p>
    <a href="/auth/login">Thử lại</a>
  </div>
</body>
</html>"##
        );
        return Html(html).into_response();
    }

    let code = match params.code {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            let html = r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <title>Thiếu mã xác thực · ezRouter</title>
  <style>
    body { font-family: system-ui, sans-serif; background: #080b10; color: #f3f4f6; min-height: 100vh; display: grid; place-items: center; padding: 20px; }
    .card { max-width: 420px; width: 100%; padding: 32px; border: 1px solid #f8717130; border-radius: 16px; background: #111622; text-align: center; }
    h1 { color: #f87171; font-size: 20px; margin-bottom: 12px; }
    p { color: #94a3b8; font-size: 13px; line-height: 1.6; margin-bottom: 20px; }
    a { color: #818cf8; text-decoration: none; font-size: 13px; font-weight: 600; }
  </style>
</head>
<body>
  <div class="card">
    <h1>❌ Thiếu Authorization Code</h1>
    <p>Không nhận được mã ủy quyền từ Google.</p>
    <a href="/auth/login">Thử lại</a>
  </div>
</body>
</html>"##;
            return Html(html).into_response();
        }
    };

    let redirect_uri = match state
        .account_pool
        .validate_and_consume_state(params.state.as_deref())
    {
        Ok(uri) => uri,
        Err(e) => {
            let err_text = html_escape(&e.to_string());
            let html = format!(
                r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <title>Lỗi Xác Thực · ezRouter</title>
  <style>
    body {{ font-family: system-ui, sans-serif; background: #080b10; color: #f3f4f6; min-height: 100vh; display: grid; place-items: center; padding: 20px; }}
    .card {{ max-width: 420px; width: 100%; padding: 32px; border: 1px solid #f8717130; border-radius: 16px; background: #111622; text-align: center; }}
    h1 {{ color: #f87171; font-size: 18px; margin-bottom: 12px; }}
    p {{ color: #94a3b8; font-size: 13px; line-height: 1.6; margin-bottom: 18px; }}
    a {{ color: #818cf8; text-decoration: none; font-size: 13px; }}
  </style>
</head>
<body>
  <div class="card">
    <h1>❌ Lỗi xác thực Google</h1>
    <p>{err_text}</p>
    <a href="/auth/login">Thử lại</a>
  </div>
</body>
</html>"##
            );
            return Html(html).into_response();
        }
    };

    match state
        .account_pool
        .exchange_oauth_code(&code, &redirect_uri, None, None)
        .await
    {
        Ok((_id, email, is_new)) => {
            let encoded_email = account::urlencoding_encode(&email);
            let redirect_target = if !target_origin.is_empty() {
                format!("{target_origin}/auth/success?email={encoded_email}&is_new={is_new}")
            } else {
                format!("/auth/success?email={encoded_email}&is_new={is_new}")
            };
            Redirect::to(&redirect_target).into_response()
        }
        Err(e) => {
            let err_text = html_escape(&e.to_string());
            let html = format!(
                r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <title>Lỗi Token · ezRouter</title>
  <style>
    body {{ font-family: system-ui, sans-serif; background: #080b10; color: #f3f4f6; min-height: 100vh; display: grid; place-items: center; padding: 20px; }}
    .card {{ max-width: 420px; width: 100%; padding: 32px; border: 1px solid #f8717130; border-radius: 16px; background: #111622; text-align: center; }}
    h1 {{ color: #f87171; font-size: 18px; margin-bottom: 12px; }}
    pre {{ text-align: left; background: #080b10; padding: 12px; border-radius: 8px; font-size: 11px; overflow-x: auto; color: #fca5a5; margin-bottom: 18px; }}
    a {{ color: #818cf8; text-decoration: none; font-size: 13px; }}
  </style>
</head>
<body>
  <div class="card">
    <h1>❌ Xác thực Google thất bại</h1>
    <pre>{err_text}</pre>
    <a href="/auth/login">Thử lại</a>
  </div>
</body>
</html>"##
            );
            Html(html).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthSuccessQuery {
    pub email: Option<String>,
    pub is_new: Option<bool>,
}

pub async fn auth_success(Query(params): Query<AuthSuccessQuery>) -> Response {
    let email = params
        .email
        .unwrap_or_else(|| "Tài khoản Google".to_string());
    let email_esc = html_escape(&email);
    let is_new = params.is_new.unwrap_or(true);

    let (heading, sub_desc, icon, border_color, badge_color, badge_text_color) = if is_new {
        (
            "Kết nối thành công!",
            "Tài khoản Google mới đã được thêm và sẵn sàng trong pool của ezRouter.",
            "✓",
            "#42d39235",
            "#42d39210",
            "#a7f3d0",
        )
    } else {
        (
            "Đã cập nhật tài khoản!",
            "Tài khoản Google đã có trong pool — đã làm mới refresh token (không tạo trùng tài khoản).",
            "↻",
            "#f59e0b35",
            "#f59e0b15",
            "#fde68a",
        )
    };

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
  <title>Kết nối thành công · ezRouter</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Be+Vietnam+Pro:wght@400;500;600;700&display=swap" rel="stylesheet">
  <style>
    :root {{ --bg: #080b10; --card: #0f1520; --line: #202836; --text: #f3f4f6; --green: #42d392; }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: 'Be Vietnam Pro', system-ui, sans-serif;
      min-height: 100vh;
      display: grid;
      place-items: center;
      padding: 24px;
      background: radial-gradient(circle at 50% 20%, #42d39218, transparent 65%), var(--bg);
      color: var(--text);
    }}
    .card {{
      width: min(420px, 100%);
      padding: 36px 30px;
      border: 1px solid {border_color};
      border-radius: 18px;
      background: var(--card);
      box-shadow: 0 25px 60px rgba(0,0,0,0.6);
      text-align: center;
    }}
    .check-circle {{
      width: 52px;
      height: 52px;
      margin: 0 auto 18px;
      border-radius: 50%;
      background: {badge_color};
      border: 2px solid {badge_text_color};
      color: {badge_text_color};
      display: grid;
      place-items: center;
      font-size: 24px;
      font-weight: 700;
    }}
    h1 {{
      font-size: 20px;
      font-weight: 700;
      color: #fff;
      margin-bottom: 8px;
    }}
    .email-chip {{
      display: inline-block;
      padding: 6px 14px;
      margin: 12px 0 16px;
      border-radius: 99px;
      border: 1px solid {border_color};
      background: {badge_color};
      color: {badge_text_color};
      font-size: 13px;
      font-weight: 600;
    }}
    p.sub {{
      font-size: 12px;
      color: #94a3b8;
      line-height: 1.6;
    }}
    .countdown {{
      margin-top: 20px;
      font-size: 11px;
      color: #64748b;
    }}
  </style>
</head>
<body>
  <div class="card">
    <div class="check-circle">{icon}</div>
    <h1>{heading}</h1>
    <div class="email-chip">{email_esc}</div>
    <p class="sub">{sub_desc}</p>
    <p class="countdown" id="cd">Cửa sổ sẽ tự động đóng sau 3s...</p>
  </div>
  <script>
    try {{
      if (window.opener) {{
        window.opener.postMessage({{ type: 'ag-account-added', email: '{email_esc}', is_new: {is_new} }}, '*');
      }}
    }} catch(e) {{}}
    let s = 3;
    const t = setInterval(() => {{
      s--;
      const el = document.getElementById('cd');
      if (el) el.textContent = 'Cửa sổ sẽ tự động đóng sau ' + s + 's...';
      if (s <= 0) {{
        clearInterval(t);
        window.close();
      }}
    }}, 1000);
  </script>
</body>
</html>"##
    );

    Html(html).into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_origin() {
        let redir = "http://127.0.0.1:20229/auth/callback";
        assert!(is_allowed_origin("http://localhost:3000", redir));
        assert!(is_allowed_origin("https://localhost", redir));
        assert!(is_allowed_origin("http://127.0.0.1:20229", redir));
        assert!(is_allowed_origin("https://router.namhv.vip", redir));
        assert!(is_allowed_origin("https://ag.namhv.vip", redir));

        assert!(!is_allowed_origin("https://attacker.example", redir));
        assert!(!is_allowed_origin("https://evil.com", redir));
        assert!(!is_allowed_origin("javascript:alert(1)", redir));
        assert!(!is_allowed_origin("", redir));
        assert!(!is_allowed_origin("not-a-url", redir));
    }
}
