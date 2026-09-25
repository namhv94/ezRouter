# ezRouter Community Launch Kit

## 1. Reddit Post Drafts

### r/rust
**Title:**
`I built ezRouter: An ultra-fast, single-binary LLM proxy in Rust replacing Python LiteLLM (sub-ms latency, Axum + embedded React UI)`

**Body:**
```markdown
Hey r/rust!

Over the last few months, my team relied on Python-based LLM proxies (like LiteLLM) to route coding agent requests (Hermes, Claude Code, Cursor) across Gemini and OpenAI Codex. However, we repeatedly ran into high latency, GIL bottlenecks during streaming, and fragile dependency trees.

So I wrote **ezRouter** (https://github.com/namhv94/ezRouter) — an ultra-lightweight, memory-safe LLM gateway built purely in Rust (Tokio + Axum + embedded SQLite + embedded React management console).

### What it does:
1. **Drop-in OpenAI Gateway:** Exposes `/v1/chat/completions` and `/v1/models`. Sub-millisecond local routing overhead.
2. **Multi-Account Quota Pooling:** Google OAuth and OpenAI Codex multi-account rotation with automated token refresh and instant failover on HTTP 429/rate limits.
3. **Token Saver Engine:** Intelligently optimizes verbose ASTs, diffs, and terminal logs to cut context token consumption by up to 60%.
4. **Single Standalone Binary (<9MB):** The React/TypeScript management console and SQLite engine are compiled directly into the binary. Zero Python runtime, zero separate containers needed.

You can install it with a 1-liner on Linux:
```bash
curl -fsSL https://raw.githubusercontent.com/namhv94/ezRouter/main/install.sh | bash
ezrouter
```
Or run via Docker: `docker run -p 20229:20229 ghcr.io/namhv94/ezrouter:latest`

Source code: https://github.com/namhv94/ezRouter
Would love feedback on the architecture, connection pooling, and Token Saver algorithms!
```

---

### r/LocalLLaMA & r/ChatGPTCoding
**Title:**
`ezRouter: A Rust-native LLM Proxy with Multi-Account Gemini/Codex Quota Pooling & 60% Token Saver`

**Body:**
```markdown
If you are using Cursor, Claude Code, or autonomous coding agents and constantly hitting rate limits or spending too much on tokens, check out **ezRouter**:

👉 Repo: https://github.com/namhv94/ezRouter

Key features for developers:
- **Pool multiple free/paid accounts:** Rotate through multiple Google Antigravity / Gemini or OpenAI Codex accounts with automated OAuth token refresh.
- **Token Saver:** Removes repetitive syntax bloat and cleans up terminal logs before sending them to the LLM, cutting costs/context by up to 60%.
- **Sub-ms Overhead:** Pure Rust binary, negligible CPU & RAM usage (~15MB RAM).
- **Embedded Web UI:** Real-time request pipeline flow and stats right out of the box.

Open source under MIT License. Check it out and let me know your thoughts!
```

---

## 2. Hacker News (Show HN)

**Title:**
`Show HN: ezRouter – High-performance LLM proxy and account pooling router in Rust`

**URL:**
`https://github.com/namhv94/ezRouter`

**First Comment (from author):**
```markdown
Hi HN!

I built ezRouter (https://github.com/namhv94/ezRouter) after getting frustrated with the complexity and memory footprint of Python-based LLM proxies. When running local agents and coding assistants like Cursor and Hermes, we needed:
1. Sub-millisecond routing latency without Python GIL locks during heavy SSE streaming.
2. Intelligent quota pooling across multiple Gemini and Codex accounts to avoid disruption.
3. A built-in Token Saver to keep massive agent contexts lean.

ezRouter is packaged as a single <9MB standalone binary containing Tokio, Axum, embedded SQLite, and a self-hosted React admin dashboard.

Architecture doc and benchmarks are available in the repo. Happy to answer any questions about the routing internals and token compression logic!
```

---

## 3. X / Twitter Thread

**Tweet 1:**
```
🚀 Introducing ezRouter: An ultra-fast, memory-safe LLM proxy & router written in 100% Rust.

Drop-in OpenAI gateway, multi-account quota pooling (Gemini & Codex), Token Saver engine, and an embedded Web UI—all in a single <9MB binary.

Star on GitHub: https://github.com/namhv94/ezRouter 👇
```

**Tweet 2:**
```
⚡ Why ezRouter?
- Sub-ms local routing overhead (Axum + Tokio)
- Pool multiple Google & Codex accounts with auto token refresh
- Token Saver cuts context usage by up to 60%
- Zero Python dependencies / Zero runtime bloat
- Beautiful live request flow dashboard built-in
```

**Tweet 3:**
```
Try it in 5 seconds on Linux:
curl -fsSL https://raw.githubusercontent.com/namhv94/ezRouter/main/install.sh | bash

Docker:
docker run -p 20229:20229 ghcr.io/namhv94/ezrouter:latest

Feedback and PRs welcome! ⭐
```

---

## 4. Bài đăng Cộng đồng Dev Việt Nam (J2TEAM Community / Rust Vietnam)

**Tiêu đề:**
`Chia sẻ dự án Open-Source: ezRouter – LLM Proxy & Router hiệu năng cao viết bằng Rust`

**Nội dung:**
```markdown
Chào anh em,

Trong quá trình xây dựng coding agent và dùng AI hỗ trợ lập trình (Cursor, Claude Code, Hermes), team mình gặp 2 bài toán lớn:
1. Rate limit & Quota: Dùng 1 tài khoản rất nhanh bị nghẽn (429), trong khi proxy Python (như LiteLLM) khá nặng nề, khởi động lâu và ngốn RAM.
2. Chi phí token: Lịch sử hội thoại của coding agent dài hàng trăm ngàn token do chứa nhiều AST, git diff, terminal log trùng lặp.

Mình quyết định viết **ezRouter** thuần bằng Rust (Tokio + Axum):
- **Tốc độ cực nhanh**: Độ trễ routing sub-millisecond, RAM chỉ tốn ~15MB.
- **Xoay vòng tài khoản tự động**: Quota pooling nhiều tài khoản Google (Gemini Thinking/Antigravity) và OpenAI Codex, tự động refresh token OAuth.
- **Token Saver Engine**: Tự động nén và tối ưu hóa context, tiết kiệm tới 60% token.
- **Single Binary (<9MB)**: Tích hợp sẵn SQLite và Web Dashboard React/Vite ngay trong file chạy, không cần cài Python hay runtime rườm rà.

Mã nguồn mở 100% MIT:
👉 GitHub: https://github.com/namhv94/ezRouter
👉 Release v0.1.0: Đã có sẵn binary Linux x86_64 và Docker container.

Anh em quan tâm ghé qua cho mình xin 1 ⭐ Star để cổ vũ tinh thần dự án open-source made in Vietnam nhé! Rất mong nhận được góp ý của mọi người.
```
