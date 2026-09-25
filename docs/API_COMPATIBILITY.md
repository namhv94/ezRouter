# API Compatibility Matrix: Python ag-proxy vs ag-proxy-rust

## 1. Endpoints

| Endpoint | Method | Python ag-proxy (prod: 20129) | ag-proxy-rust (staging: 20229) | Status | Notes |
|---|---|---|---|---|---|
| `/` | GET | Status / UI | Staging banner | Planned | Scaffold has basic text banner |
| `/health` | GET | Health JSON | Health JSON | Implemented (Scaffold) | Returns status, mode, port, data dir |
| `/v1/models` | GET | Models list JSON | Models list JSON | Planned | Will return supported Antigravity models |
| `/v1/chat/completions` | POST | OpenAI chat completions (sync + stream) | OpenAI chat completions (sync + stream) | Non-streaming Implemented (Phase 2B) | Streaming rejected with clean 400 in 2B; mock provider for staging/test |
| `/admin/*` | Various | Web admin interface & auth | Web admin interface | Planned | Phase 3 |

## 2. Authentication & Headers
- `Authorization: Bearer <API_KEY>` compatibility maintained.
- OpenAI client libraries (Python, Node.js, curl) pointing to `http://127.0.0.1:20229/v1` will function identically without client code modifications once endpoints are implemented.

## 3. Strict Port and Data Isolation
- `20129`: Production Python `ag-proxy`.
- `20229`: Staging Rust `ag-proxy-rust`.
- The Rust binary refuses to bind to `20129` or use `~/.ag-proxy`.
