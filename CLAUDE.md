# CLAUDE.md

Project-level instructions for Claude when working in this repository.

## What this project is

`gmaps-cli` (binary name: `gmaps`) is a **Rust** CLI for the Google Maps
Platform — Geocoding, Places API (New), and Routes API. It targets
**macOS only** because GPS uses CoreLocation via `objc2`, and CoreLocation
authorization requires the binary to live inside an `.app` bundle.

The repository was previously a TypeScript/Bun implementation. **All Bun
content has been removed.** The current codebase is 100% Rust; do not write
Node/Bun code here.

## Toolchain & build

This project uses a Nix flake (`flake.nix`) + direnv to pin the toolchain.
Inside the dev shell you have `cargo`, `clippy`, `rustfmt`, `cargo-bundle`,
`cargo-nextest`, and the macOS SDK on PATH.

- `cargo build --release` — plain binary at `target/release/gmaps` (no GPS)
- `./scripts/build.sh` — builds the `.app` bundle with ad-hoc code signing;
  required for any `-H` / `--here` GPS feature
- `cargo test` — unit tests (no integration tests live in `tests/` yet)
- `cargo clippy --all-targets` — lint
- `cargo fmt` — format
- `cargo audit` — checks the lock file against the RustSec advisory DB

## Architectural conventions

- **One `MapsClient` per command invocation** (`http::MapsClient::from_config`).
  Don't open a new client mid-flight.
- **Errors propagate via `?`.** Don't add `process::exit(1)` from inside
  command modules — `main::print_error` is the single rendering site, and it
  knows how to surface `MapsApiError` codes.
- **API errors come in two flavors:**
  - `MapsApiError::Api { status, message, code }` — non-2xx HTTP
  - `MapsApiError::Logical { message, code }` — HTTP 200 but the body said
    `ZERO_RESULTS` / `OVER_QUERY_LIMIT` / etc.
    Pick the right one when wrapping; don't fake `status: 200` on `::Api`.
- **Config writes are atomic.** Always go through
  `config::write_private_file()`, never `fs::write`. The helper handles
  mode 0600 + rename-over-target so a crash mid-write can't corrupt
  `config.yaml` or leak secrets.
- **API keys never log in plaintext.** `Config` has a manual `Debug` impl
  that masks the key, and `http::redact_pii` strips coordinates / addresses
  from `DEBUG=1` output. New code that touches request/response bodies
  should route through `redact_pii` if it ends up on stderr.
- **CLI flags use `clap::ValueEnum`** when the value is a closed set
  (`--mode`, `--rank`). Open-ended sets (Google place types, place names)
  stay as `String`.

## Patterns to follow

- Add new shared rendering helpers to `src/commands/util.rs`
  (`new_table()`, `print_json()` already live there).
- New API call modules go under `src/api/`, with one file per Google API
  surface area, keeping request/response types private.
- `LatLng` is the canonical coordinate type; don't reintroduce
  `(f64, f64)` tuples in public APIs.
- Use `anyhow::Result` at command-module boundaries, `MapsApiError` at the
  `api/` and `http` layer. Convert with `?` — `MapsApiError` already has
  `From` for `anyhow::Error` via `thiserror`.

## Patterns to avoid

- `serde_yaml` (deprecated upstream) — use `serde_yaml_ng`.
- `OpenSSL` features in `reqwest` — we use `rustls-tls`.
- `eprintln!` + `std::process::exit(1)` from command code (see "Errors
  propagate via `?`" above).
- New `#[allow(dead_code)]` markers without a TODO + a near-term plan.
- Adding crates with non-permissive licenses (GPL / AGPL); the project
  ships under MIT.

## Testing

- Unit tests live alongside the code in `#[cfg(test)] mod tests` blocks.
- Async tests use `#[tokio::test]`; the runtime macros feature is already
  enabled.
- Network-dependent tests must point at RFC 6761 reserved hosts
  (`*.invalid`) so they fail offline-deterministically rather than hitting
  the real Google APIs.
- When fixing a security bug, add a regression test that exercises the
  pre-fix failure mode (see
  `http::tests::legacy_network_error_does_not_leak_api_key`).

## Commit style

Conventional Commits: `fix:`, `feat:`, `chore:`, `refactor:`, `test:`,
`docs:`. Subjects under ~70 characters; bodies wrap at ~80. Each commit
should compile and pass `cargo test` on its own — bisect-friendly history
matters.

## Files Claude usually shouldn't touch without asking

- `flake.nix` / `flake.lock` — pinned toolchain; bumping affects every
  developer's shell.
- `Cargo.lock` — touch only as a side effect of a deliberate dependency
  change in `Cargo.toml`.
- `resources/Info.plist.ext` — affects macOS TCC (Location Services)
  permission strings.
- `LICENSE` — no edits without an explicit relicensing request.
