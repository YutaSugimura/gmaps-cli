# Contributing to gmaps-cli

Thanks for your interest! Bug reports, fixes, and small enhancements are
all welcome. This document covers the day-to-day workflow.

By participating in this project you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Before you start

- **Search existing issues** before opening a new one — your idea may
  already be discussed.
- **For non-trivial features**, open an issue first to align on scope
  before writing code. PRs that add a flag or a new subcommand without
  prior discussion may end up rejected for reasons that would have been
  cheaper to surface earlier.
- **macOS only**: this project ships against CoreLocation via
  `objc2-core-location`. Cross-platform PRs are out of scope for now.

## Development environment

The repo ships a reproducible toolchain via Nix Flakes + direnv. The
fastest path:

```bash
git clone https://github.com/YutaSugimura/gmaps-cli.git
cd gmaps-cli
direnv allow            # or: nix develop
```

This puts the pinned `cargo` 1.95, `clippy`, `rustfmt`, `cargo-bundle`,
`cargo-nextest`, and the macOS SDK on PATH. If you don't use Nix, install
Rust 1.95+ yourself and `cargo install cargo-bundle` for the `.app` flow.

## Running the checks

These are the same checks CI runs:

```bash
cargo fmt --all -- --check        # formatting
cargo clippy --all-targets -- -D warnings   # lint, warnings as errors
cargo test                        # unit tests
cargo audit                       # advisory database (security)
```

If you're touching anything in `src/api/` or `src/commands/`, also try a
real run end-to-end against a sandbox API key — the unit suite doesn't
hit Google's servers.

## Coding conventions

The full set of conventions Claude (and humans) should follow is in
[CLAUDE.md](CLAUDE.md). The highlights:

- **Errors propagate via `?`.** Don't reintroduce `eprintln!` +
  `std::process::exit(1)` from inside command modules. `main::print_error`
  is the single rendering site.
- **API key never logs in plaintext.** Use `mask_api_key`, the manual
  `Debug` impl on `Config`, and `redact_pii` for any new request/response
  body that touches stderr.
- **Atomic file writes only.** Anything that persists user data goes
  through `config::write_private_file()`, which guarantees mode 0600 and
  rename-over-target.
- **`clap::ValueEnum` for closed-set flags.** Open-ended sets stay as
  `String`.
- **No GPL/AGPL dependencies** — the project ships under MIT and we
  intend to keep it that way.

## Commit style

We use [Conventional Commits](https://www.conventionalcommits.org):

```
fix(http): strip URL from reqwest errors to prevent API key leak
feat(nearby): support --open-now for searchText
docs(readme): document --depart timezone semantics
```

Subjects under ~70 characters. Bodies wrap at ~80 and explain _why_, not
_what_ (the diff already shows what). Each commit should compile and
pass `cargo test` on its own — bisect-friendly history matters.

## Pull requests

1. Branch off `main`. We don't use a `develop` branch.
2. Keep PRs focused. One logical change per PR; reviewers shouldn't have
   to context-switch between unrelated fixes.
3. Update or add tests. Security fixes require a regression test that
   exercises the pre-fix failure mode.
4. Update `CHANGELOG.md` under the `[Unreleased]` section.
5. Run `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`
   locally before opening the PR.

## Reporting security issues

Please do **not** open a public issue for security vulnerabilities.
Instead, see [SECURITY.md](SECURITY.md) for the disclosure process.

## License

By contributing, you agree that your contributions will be licensed
under the [MIT License](LICENSE).
