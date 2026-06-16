# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `gmaps nearby --map`: render a compass-oriented ASCII map of results around
  the search center, numbered to match the table, with off-map results listed
  separately. Ignored with `--json`.

### Changed

- GPS timeout error now points to enabling location under System Settings >
  Privacy & Security > Location Services, instead of the misleading "not an
  .app bundle / run cargo bundle" advice. Docs note that `tccutil` cannot
  reset Location (use the System Settings toggle).

## [0.1.2] - 2026-06-16

### Added

- Homebrew install path via the `YutaSugimura/homebrew-tap` cask
  (`brew install --cask gmaps`).
- This changelog.

### Changed

- Documentation reorganization: README and INSTALL.md updated for the
  Homebrew flow, first-time setup, and clean reinstall; clarified the role
  of each doc.

### Fixed

- GPS was wrongly disabled ("Not running inside an .app bundle") when `gmaps`
  was launched through a symlink, as Homebrew and the manual `~/.local/bin`
  install both do. `is_app_bundle()` now canonicalizes the executable path
  before checking for the `.app/Contents/MacOS/` layout.

## [0.1.1] - 2026-06-16

### Added

- `scripts/build-signed.sh`: Developer ID signing + Apple notarization +
  stapling for a distributable `.app` bundle.

### Changed

- Release workflow now produces a Developer ID signed and notarized bundle
  (no Gatekeeper bypass needed) instead of ad-hoc signing.
- Updated dependencies: clap 4.6, reqwest 0.13, inquire 0.9; nixpkgs now
  tracks the stable `nixpkgs-26.05-darwin` channel.

## [0.1.0] - 2026-05-01

### Added

- Initial release: a macOS CLI for the Google Maps Platform — Geocoding,
  Places API (New), and Routes API — with CoreLocation GPS.
- Commands: `init`, `config`, `places`, `geocode`, `reverse`, `whereami`,
  `nearby`, `route`.
- Interactive setup wizard, saved places (`places.yaml`), `.app` bundle
  packaging, and the prebuilt-release installation flow (INSTALL.md).

[Unreleased]: https://github.com/YutaSugimura/gmaps-cli/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/YutaSugimura/gmaps-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/YutaSugimura/gmaps-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/YutaSugimura/gmaps-cli/releases/tag/v0.1.0
