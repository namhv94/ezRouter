# ezRouter UI probes (pwterm)

Browser automation for the admin UI using [pwterm](https://github.com/dhhieu113pro/pwterm)
(Playwright-style `browser_*` MCP tools over system Chromium, headless, Termux-safe).

## Scripts

| Script | What it does |
|---|---|
| `pwterm_mcp.py` | Reusable MCP stdio driver (JSON-RPC 2.0 over `pwterm-mcp`). |
| `ui_combo_probe.py` | Verifies the Combos save flow stores ids that **route** correctly. |

## The regression this guards against

Routing contract: a model id `X/rest` is resolved to the provider with prefix `X`,
and dispatch strips `X/` from `rest` before sending upstream. Therefore every id
offered by `/v1/models` (and by the combo picker) must be exactly
`{provider_prefix}/{stored_upstream_model}`.

`src/routes/models.rs` used to keep a stored model as-is when it already started
with the provider prefix, listing `nvidia/nemotron-...` for the `nvidia` provider.
Saving that into a combo produced requests that 404'd (the strip ate the required
`nvidia/` namespace) — "wrong name in route".

## Usage

```sh
# read-only: check every offered id satisfies the round-trip invariant
AG_ADMIN_KEY=123456 python3 scripts/ui-probe/ui_combo_probe.py

# restrict to ids containing 'nvidia', and also run the UI create/save/delete probe
AG_ADMIN_KEY=123456 python3 scripts/ui-probe/ui_combo_probe.py \
    --filter nvidia --save-probe
```

Requires `pwterm-mcp` on PATH (`pkg`-installed launcher from the pwterm repo)
and the ezRouter dashboard running at `--base-url` (default
`http://127.0.0.1:20229`). The admin key defaults to `AG_ADMIN_KEY`.

## Running the Rust tests on Termux

Android/Termux has no `/tmp`, and several unit tests create SQLite files in the
system temp dir. Point `TMPDIR` at the Termux tmp or they fail with
`SqliteFailure ... unable to open database file: /tmp/...`:

```sh
TMPDIR=/data/data/com.termux/files/usr/tmp cargo test --release
```
