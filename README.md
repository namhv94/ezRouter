# ⚡ ezRouter (Termux fork)

Fork of [namhv94/ezRouter](https://github.com/namhv94/ezRouter) — an ultra-fast LLM router & proxy written in Rust (multi-account quota pooling for Gemini / Codex / Claude, Token Saver engine, embedded Web UI).

👉 See the **[original repo](https://github.com/namhv94/ezRouter)** for full documentation, features, benchmarks, and releases.

## Why this fork exists

The upstream prebuilt binaries and installer target **Linux x86_64** only. This fork documents how to **build and run ezRouter natively on an Android phone via Termux** (aarch64).

## Run on Android (Termux)

```bash
# 1. Toolchain
pkg update
pkg install git rust binutils

# 2. Build
git clone https://github.com/dhhieu113pro/ezRouter.git
cd ezRouter
cp .env.example .env
cargo build --release   # 10–40 min on a phone
```

Verify: `curl http://127.0.0.1:20229/health` · Console: `http://127.0.0.1:20229/`

# 3. Install as a global command + env file
./scripts/install-termux.sh
```

**Start it:**

```bash
ezrouter
```

Verify: `curl http://127.0.0.1:20229/health` · Console: `http://127.0.0.1:20229/`

**LAN access** is the default — the installer binds to `0.0.0.0`, so other devices can open
`http://<phone-LAN-IP>:20229/` right away. Find the phone's LAN IP with
`ifconfig | grep -oE 'inet [0-9.]+' | grep -v 127.0.0.1`.

> ⚠️ Plain HTTP on your LAN — the `AG_API_KEY` is visible to anyone on the network. Fine for home use; don't reuse a real credential.

### Configuration

The binary reads plain env vars (`AG_HOST`, `AG_PORT`, `AG_DATA_DIR`, `AG_API_KEY`,
`AG_UPSTREAM_BASE_URL`, `AG_UPSTREAM_API_KEY`, ...). The launcher loads them from
`~/.ezrouter/env` before starting, so edit that file to change settings:

```sh
AG_HOST=0.0.0.0
AG_PORT=20229
AG_DATA_DIR=/data/data/com.termux/files/home/.ezrouter/data
RUST_LOG=ezrouter=info,tower_http=info
```

After rebuilding (`cargo build --release`), refresh the installed binary with
`./scripts/install-termux.sh`.

### Run as a Termux service (auto-start, auto-restart)

With [`termux-services`](https://github.com/termux/termux-services) installed
(`pkg install termux-services`), ezRouter runs supervised by `runsv` — it starts on boot
after Termux opens, restarts on crash, and logs to `~/log/ezrouter/current`:

```bash
pkg install termux-services
./scripts/install-termux.sh   # also creates $PREFIX/var/service/ezrouter
sv-enable ezrouter            # enable + start now and on future boots
```

Manage it with `sv`:

```bash
sv status ezrouter   # check state
sv down ezrouter     # stop (use sv-disable to also skip future boots)
sv up ezrouter       # start again
sv restart ezrouter  # pick up a new config in ~/.ezrouter/env
```

Logs are rotated by `svlogd` into `~/log/ezrouter/` — use
`tail -f ~/log/ezrouter/current` to follow output.

### Manual run (no install)

Still possible from the repo root so the web UI in `ui/dist` is found:

```bash
AG_HOST=0.0.0.0 AG_PORT=20229 AG_API_KEY=123456 AG_DATA_DIR=$HOME/.ezrouter/data \
  ./target/release/ezrouter
```

## Credits

All credit for ezRouter goes to [NamHV](https://github.com/namhv94) and the [EzLabs](https://ezlabs.tech) team, released under the [MIT license](LICENSE). This fork only adds Termux documentation.
