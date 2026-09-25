# Python Production vs. Rust Staging Baseline Benchmark

## Overview & Methodology
A repeatable read-only benchmark comparing current Python production (`http://127.0.0.1:20129`) against an isolated Rust staging instance (`http://127.0.0.1:20229`).

- **Harness:** Python standard library (`http.client` with `concurrent.futures.ThreadPoolExecutor`).
- **Concurrency tiers:** 1 (500 requests), 10 (2,000 requests), 50 (5,000 requests).
- **Staging flags:** `AG_PORT=20229`, `AG_DATA_DIR=/tmp/ezrouter-benchmark`, `AG_USE_MOCK_PROVIDER=true`.
- **Target binary:** `target/debug/ag-proxy-rust`.

## Caveats
1. **Upstream Completions:** `POST /v1/chat/completions` was benchmarked **only** on Rust staging with `AG_USE_MOCK_PROVIDER=true`. Python production completions route to live billable upstreams and were omitted to prevent upstream billing and unmetered external traffic.
2. **Endpoint Equivalence:** `/health` and `/v1/models` are local proxy operations (in-memory/SQLite lookups). They reflect proxy overhead and connection handling but are **not equivalent** to real upstream completion latency, which is heavily dominated by upstream network and inference times.

---

## Results Table

| Target & Endpoint | Concurrency | Total Reqs | Success / Err | RPS | p50 (ms) | p95 (ms) | p99 (ms) | Target RSS (MB) | Target CPU (%) |
|---|---|---|---|---|---|---|---|---|---|
| **Python Prod: GET /health** | 1 | 500 | 500 / 0 | 1,261.5 | 0.77 | 0.94 | 1.04 | 92.58 | 95.9% |
| **Rust Staging: GET /health** | 1 | 500 | 500 / 0 | 3,528.7 | 0.28 | 0.34 | 0.37 | 17.34 | 56.5% |
| **Python Prod: GET /health** | 10 | 2,000 | 2,000 / 0 | 1,389.0 | 7.05 | 7.76 | 8.19 | 92.58 | 100.7% |
| **Rust Staging: GET /health** | 10 | 2,000 | 2,000 / 0 | 8,071.0 | 1.02 | 2.53 | 3.67 | 17.96 | 121.1% |
| **Python Prod: GET /health** | 50 | 5,000 | 5,000 / 0 | 1,398.8 | 34.41 | 53.72 | 56.84 | 92.73 | 99.0% |
| **Rust Staging: GET /health** | 50 | 5,000 | 5,000 / 0 | 7,911.0 | 4.23 | 12.36 | 17.67 | 19.82 | 118.7% |
| **Python Prod: GET /v1/models** | 1 | 500 | 500 / 0 | 66.3 | 15.65 | 18.77 | 22.01 | 92.76 | 35.3% |
| **Rust Staging: GET /v1/models** | 1 | 500 | 500 / 0 | 1,604.9 | 0.52 | 1.84 | 2.04 | 19.83 | 70.6% |
| **Python Prod: GET /v1/models** | 10 | 2,000 | 2,000 / 0 | 71.7 | 27.13 | 556.32 | 1,350.92 | 94.45 | 42.7% |
| **Rust Staging: GET /v1/models** | 10 | 2,000 | 2,000 / 0 | 5,880.3 | 1.36 | 3.27 | 8.71 | 19.90 | 211.7% |
| **Python Prod: GET /v1/models** | 50 | 5,000 | 4,908 / 92 | 58.1 | 290.89 | 2,862.73 | 4,568.49 | 103.55 | 38.9% |
| **Rust Staging: GET /v1/models** | 50 | 5,000 | 5,000 / 0 | 3,086.4 | 9.35 | 41.26 | 57.93 | 20.49 | 214.8% |
| **Rust Mock: POST /v1/chat/completions** | 1 | 500 | 500 / 0 | 1,037.9 | 0.69 | 1.25 | 7.58 | 20.53 | 66.4% |
| **Rust Mock: POST /v1/chat/completions** | 10 | 2,000 | 2,000 / 0 | 1,190.5 | 5.73 | 31.37 | 40.04 | 21.07 | 140.5% |
| **Rust Mock: POST /v1/chat/completions** | 50 | 5,000 | 5,000 / 0 | 969.7 | 46.22 | 108.96 | 134.48 | 23.84 | 144.3% |

---

## Observations
1. **Throughput & Concurrency Scaling:**
   - On `/health`, Rust delivers ~5.7x higher throughput (~8,000 RPS vs ~1,400 RPS) with p99 staying sub-18ms vs ~57ms on Python.
   - On `/v1/models`, Python degrades under concurrency due to synchronous database/auth lookup locking (yielding 92 errors and 4.5s p99 at c=50). Rust handles 5,000 requests with zero errors at 3,086 RPS (p99 of 57.9ms), representing a ~53x throughput improvement.
2. **Memory Utilization:**
   - Python production maintains an RSS of ~93 MB to 104 MB.
   - Rust staging operates at ~17 MB to 24 MB RSS (~4.5x lower memory footprint).
3. **Safety Verification:**
   - Production health (`http://127.0.0.1:20129/health`) returned HTTP 200 before and after testing.
   - Port `20229` confirmed released immediately after stopping the staging daemon.
