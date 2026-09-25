# ezRouter

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![API](https://img.shields.io/badge/API-OpenAI--compatible-412991)](#openai-compatible-api)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**ezRouter** is a high-performance, memory-safe LLM proxy & router written in Rust. It exposes an OpenAI-compatible API to seamlessly distribute, balance, and fail over requests across multiple backend providers (Google Gemini / Antigravity, OpenAI Codex, Claude, custom OpenAI-compatible providers) with multi-account quota pooling, automatic token refresh, and intelligent prompt token optimization (Token Saver).

Built with Tokio + Axum, ezRouter provides sub-millisecond local routing overhead, native Server-Sent Events (SSE) streaming, and an intuitive, mobile-ready live web management dashboard.

---

## Key Features

- **OpenAI-Compatible Gateway:** Drop-in replacement for OpenAI endpoints (`/v1/chat/completions`, `/v1/models`). Works out of the box with Hermes Agent, Claude Code, Cursor, Continue, LangChain, LlamaIndex, etc.
- **Provider & Account Pooling:**
  - Multi-account Google OAuth rotation with automated token refresh.
  - Multi-account OpenAI Codex pool with PKCE OAuth flow.
  - Dynamic Custom Providers (OpenAI, DeepSeek, Groq, Ollama, vLLM, etc.).
- **Smart Routing & Combo Models:**
  - Combine multiple models/providers into logical virtual models with fallback chains, round-robin, or weighted distribution.
  - Automatic error classification: immediate failover on HTTP 429 (rate limits) or 5xx without stalling the client.
- **Token Saver Engine:**
  - Integrated token optimization (RTK, Caveman, Ponytail) that reduces repetitive AST bloat, whitespace overhead, and verbose logs—cutting token usage by up to 60%.
- **Zero Python Dependencies:**
  - 100% native Rust binary with embedded SQLite (`rusqlite`) and self-hosted Vite/React management console.
- **Security & Privacy by Design:**
  - Sensitive tokens, API keys, and credentials are encrypted/masked in logs, database queries, and UI.
  - Role-based API keys with granular admin vs client access permissions.

---

## Architecture

```text
       OpenAI Clients (Hermes, Claude Code, Cursor, Python/Node SDKs)
                                      |
                                      v
                            ezRouter (Axum HTTP)
    +---------------------------------+---------------------------------+
    |  - Authentication & API Keys    |  - Token Saver (Compression)    |
    |  - Combo & Fallback Routing     |  - Live Request Logging (SQLite)|
    |  - Real-time Traffic Monitor    |  - Embedded Web Admin Console   |
    +---------------------------------+---------------------------------+
           |                          |                         |
           v                          v                         v
   Google / Gemini             OpenAI / Codex            Custom Providers
 (Multi-Account OAuth)       (Multi-Account Pool)      (DeepSeek, Groq, etc.)
```

---

## Quick Start

### 1. Clone & Build

```bash
git clone https://github.com/namhv94/ezRouter.git
cd ezRouter

# Copy example environment configuration
cp .env.example .env

# Build and run in debug mode
cargo run
```

The server binds to `127.0.0.1:20229` by default.

### 2. Verify Health

```bash
curl http://127.0.0.1:20229/health
```

### 3. Access Web Console

Open `http://127.0.0.1:20229/` in your browser. Enter your admin key (configured in `AG_API_KEY`) to access the dashboard.

---

## Production Deployment

### Build Release Binary

```bash
cargo build --release
# The compiled standalone binary is located at target/release/ezrouter
```

### systemd Service

An example systemd service unit is available in `deploy/ezrouter.service.example`:

```bash
sudo cp deploy/ezrouter.service.example /etc/systemd/system/ezrouter.service
# Update User, WorkingDirectory, and environment paths to match your server
sudo systemctl daemon-reload
sudo systemctl enable --now ezrouter
sudo systemctl status ezrouter
```

---

## Configuration

Configuration is loaded from environment variables or a `.env` file:

| Variable | Default | Description |
|---|---|---|
| `AG_HOST` | `127.0.0.1` | Bind address (`0.0.0.0` for all interfaces) |
| `AG_PORT` | `20229` | Port to listen on |
| `AG_DATA_DIR` | `./data` | Directory for SQLite database and account configs |
| `AG_API_KEY` | `ag-proxy-key` | Master admin API key |
| `AG_OAUTH_REDIRECT_URI` | `http://localhost:<port>/auth/callback` | OAuth redirect URI for Google account auth |
| `AG_GOOGLE_CLIENT_ID` | unset | Google OAuth Client ID |
| `AG_GOOGLE_CLIENT_SECRET` | unset | Google OAuth Client Secret |
| `EZ_ALLOWED_DOMAINS` | unset | Comma-separated list of additional allowed CORS/OAuth domains |
| `RUST_LOG` | `ezrouter=info,tower_http=info` | Tracing filter level |
| `AG_UPSTREAM_CONNECT_TIMEOUT_SECS` | `10` | Timeout connecting to upstreams (seconds) |
| `AG_UPSTREAM_READ_TIMEOUT_SECS` | `60` | Stream read timeout (seconds) |
| `AG_UPSTREAM_REQUEST_TIMEOUT_SECS` | `120` | Full request timeout (seconds) |

---

## OpenAI-Compatible API

Configure your AI clients to point to ezRouter:

- **Base URL:** `http://127.0.0.1:20229/v1`
- **API Key:** Any API key created in the ezRouter Admin Console (or `AG_API_KEY`).

### Python Example

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://127.0.0.1:20229/v1",
    api_key="your-ezrouter-api-key"
)

response = client.chat.completions.create(
    model="ag/gemini-3.8-flash-high",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Hello!"}
    ],
    stream=True
)

for chunk in response:
    content = chunk.choices[0].delta.content or ""
    print(content, end="", flush=True)
print()
```

### cURL Example

```bash
curl http://127.0.0.1:20229/v1/chat/completions \
  -H "Authorization: Bearer your-ezrouter-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "ag/gemini-3.8-flash-high",
    "messages": [{"role": "user", "content": "Explain quantum computing briefly."}],
    "stream": false
  }'
```

---

## Web UI Development

The web console is built using React, TypeScript, and Vite. Assets are automatically bundled into the Rust binary at compile time.

```bash
cd ui
npm install
npm run dev     # Start Vite dev server
npm run build   # Build production assets into ui/dist
```

---

## License

This project is licensed under the [MIT License](LICENSE).
