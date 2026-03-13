# Agent Instruction

## AI Behavior & Persona
- The user is an experienced software engineer. Treat them as such—skip over-explaining basic concepts.
- **Simple Tasks**: For straightforward, zero-decision tasks (e.g., deleting obsolete code, adding simple implementations), execute them directly without waiting for approval.
- **Complex Tasks**: For challenging or architecturally significant tasks, ALWAYS provide a design or implementation plan for review BEFORE writing code.

## Git & Version Control
- **Commit Style**: ALWAYS break changes into small, atomic, and focused commits. Never lump multiple unrelated changes or large features into a single commit.
- **Commit Messages**: Follow Conventional Commits format (e.g., `feat: ...`, `fix: ...`, `chore(backend): ...`, `refactor(test): ...`).
- **RESTRICTION**: DO NOT create commits automatically unless explicitly requested by the user. NEVER push.

## Project Overview
- **dandelion** is a programmable network proxy written in Rust.
- Routing rules are defined in [Rune](https://github.com/rune-rs/rune) scripts (`.rn` files), not static config.
- Single-threaded Tokio runtime (`current_thread` flavor) — Rune objects aren't `Send`, so all concurrency uses `spawn_local` / `Rc`.

## Project Structure
- `core/` — The entire Rust crate (workspace root for Cargo is `core/`).
  - `core/Cargo.toml` — All dependencies live here.
  - `core/src/lib.rs` — Crate root, exports `config` and `core` modules.
  - `core/src/bin/dandelion.rs` — CLI binary entry point. Uses `structopt` for args, `flexi_logger` for logging.
  - `core/src/config/` — Rune scripting engine integration.
    - `engine/mod.rs` — `Engine` struct: loads Rune config, binds acceptors, runs event loop.
    - `engine/connect.rs` — Rune-exposed connector functions (`new_tcp_async`, `new_tls_async`, etc.).
    - `engine/resolver.rs` — Rune-exposed DNS resolver creation (`create_system_resolver`, `create_udp_resolver`).
    - `engine/geoip.rs` — GeoIP MMDB loading from file or URL with caching.
    - `engine/iplist.rs` — CIDR-based IP network set matching.
    - `engine/tun.rs` — TUN-mode fake DNS resolver and DNS handler.
    - `engine/testing.rs` — Test harness for running Rune code snippets in tests.
    - `rune.rs` — `create_wrapper!` macro for Rune type wrappers.
  - `core/src/core/` — Low-level network primitives.
    - `endpoint.rs` — `Endpoint` enum (domain:port or socket addr).
    - `io.rs` — `Io` trait (AsyncRead + AsyncWrite + Unpin + Send + Debug).
    - `acceptor/` — Inbound: `http.rs` (HTTP proxy CONNECT + plain), `socks5.rs`.
    - `connector/` — Outbound: `tcp.rs` (Happy Eyeballs RFC 8305), `tls.rs` (native-tls), `http.rs` (HTTP CONNECT), `socks5.rs`, `quic.rs`, `simplex.rs` (WebSocket tunnel), `block.rs` (deny), `speed.rs` (race connectors).
    - `resolver/` — DNS: `system.rs` (dns-lookup), `hickory.rs` (Hickory UDP).
    - `quic/` — QUIC via Quinn: `client.rs`, `mod.rs` (QuicStream).
    - `simplex/` — WebSocket tunneling: `client.rs`, `server.rs`, `io.rs` (WS↔AsyncRead/Write adapter).
    - `tun/` — TUN device: `device.rs`, `resolver.rs` (fake DNS with LRU).
- `Dockerfile` — Multi-stage build with `cargo-chef`. Runtime base: Debian.
- `snapcraft.yaml` — Snap package config.
- `.github/workflows/ci.yml` — CI: fmt, clippy, test on macOS/Windows/Ubuntu + Docker image build on main.
- `.github/workflows/release.yml` — Release: GitHub release, cross-compiled binaries, Docker image, macOS GUI app.

## Build & Test Commands
All Cargo commands must be run from `/core` or use `--manifest-path core/Cargo.toml`:
```sh
cargo fmt --all --manifest-path core/Cargo.toml -- --check
cargo clippy --all-targets --all-features --manifest-path core/Cargo.toml -- -D warnings
cargo test --manifest-path core/Cargo.toml -- --include-ignored
cargo build --manifest-path core/Cargo.toml
```

## Code Conventions
- **Error handling**: Use `anyhow::Result` throughout. The crate re-exports `pub use anyhow::{Error, Result}` from `lib.rs`.
- **Async patterns**: All async code runs on single-threaded Tokio. Use `Rc` instead of `Arc` for shared state within tasks. Use `spawn_local` not `spawn`.
- **Rune integration**: New types exposed to Rune use `#[derive(Any)]` and register via `Module`. Use the `create_wrapper!` macro in `config/rune.rs` for wrapping trait objects or inner types.
- **Naming**: Rune-exposed async functions use `path = fn_name_async` attribute for the Rune-side name but regular `fn` names in Rust.
- **Testing**: Use the `testing::run()` harness in `config/engine/testing.rs` for testing Rune-exposed APIs. Use `rstest` for parameterized tests.
- **Logging**: Use `tracing` macros (`info!`, `debug!`, `warn!`, `error!`) with the `log` feature bridge. CLI uses `flexi_logger`.

