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

# 3. Run (from the repo root so the web UI in ui/dist is found)
./target/release/ezrouter
```

Verify: `curl http://127.0.0.1:20229/health` · Console: `http://127.0.0.1:20229/`

**Configure & start** (the `.env` file is **not** auto-loaded — pass variables inline):

```bash
mkdir -p ~/.ezrouter
AG_HOST=127.0.0.1 \
AG_PORT=20229 \
AG_API_KEY=123456 \
AG_DATA_DIR=$HOME/.ezrouter \
./target/release/ezrouter
```

**Access from other devices on your LAN** — bind to all interfaces and open `http://<phone-LAN-IP>:20229/`:

```bash
AG_HOST=0.0.0.0 AG_PORT=20229 AG_API_KEY=123456 AG_DATA_DIR=$HOME/.ezrouter \
  ./target/release/ezrouter &
```

Find the phone's LAN IP with `ifconfig | grep -oE 'inet [0-9.]+' | grep -v 127.0.0.1`.

> ⚠️ Plain HTTP on your LAN — the `AG_API_KEY` is visible to anyone on the network. Fine for home use; don't reuse a real credential.

**Reusable alias** (add to `~/.bashrc`):

```bash
alias ezrouter='AG_DATA_DIR=$HOME/.ezrouter ezrouter'
```

**Tips**

- `termux-wake-lock` keeps it running with the screen off; use `tmux` to survive closing the session.
- `cp target/release/ezrouter $PREFIX/bin/ezrouter` to run `ezrouter` from anywhere.
- Stop the running instance with `pkill -f target/release/ezrouter`.

## Credits

All credit for ezRouter goes to [NamHV](https://github.com/namhv94) and the [EzLabs](https://ezlabs.tech) team, released under the [MIT license](LICENSE). This fork only adds Termux documentation.
