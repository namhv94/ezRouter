# ezRouter Architecture

## 1. Overview
`ezRouter` is a high-performance, memory-safe LLM proxy & router built in Rust. It exposes an OpenAI-compatible HTTP interface while intelligently orchestrating, load balancing, and failing over across multiple backend providers (Google Gemini / Antigravity, OpenAI Codex, Claude, Custom Proxies) with automated quota tracking and real-time token optimization.

## 2. Core Principles
- **Blazing Fast & Low Latency:** Written in 100% Rust on top of Tokio and Axum with zero-copy stream processing.
- **Failover & High Availability:** Automatically detects rate limits (HTTP 429), server errors (5xx), and network timeouts to smoothly fall back to secondary accounts or providers.
- **Fair Rotation & Pooling:** Distributes requests fairly across pooled accounts using LRU and quota-aware scheduling.
- **Token Saver Pipeline:** Integrated prompt compression algorithms (RTK, Caveman, Ponytail) that strip redundant AST boilerplate and reduce token consumption by up to 60%.
- **Zero Python Dependencies:** Self-contained single binary with embedded SQLite and modern React admin dashboard.

## 3. Component Architecture
1. **HTTP Routing Layer (Axum):**
   - OpenAI compatible endpoints: `/v1/chat/completions`, `/v1/models`, `/v1/responses`.
   - Admin REST & Server-Sent Events (SSE): `/admin/*`, `/health`, live traffic monitor.
2. **Provider & Transport Layer:**
   - Multi-account Google OAuth rotation with automated token refresh.
   - OpenAI Codex OAuth & session management with native streaming and fallback.
   - Combo model router (weighted, round-robin, fallback chains).
3. **Token Saver Engine:**
   - Heuristic truncation, whitespace normalization, repeated sequence deduplication.
   - Preserves core instructions while reducing prompt bloat.
4. **Persistence Layer (SQLite):**
   - Embedded database (`data.sqlite`) managing accounts, providers, API keys, usage statistics, and request logs.
