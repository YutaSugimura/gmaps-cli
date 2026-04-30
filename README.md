# gmaps-cli (`gmaps`)

[![CI](https://github.com/YutaSugimura/gmaps-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/YutaSugimura/gmaps-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](#supported-platforms)

A command-line interface for Google Maps Platform — nearby search, directions,
geocoding, and GPS-driven location lookups, all from your terminal.

> **Supported OS**: macOS only (CoreLocation via `objc2`)
> **Language**: Rust 1.95+, distributed as a single `.app` bundle
> **Command**: `gmaps`

## Supported platforms

macOS 12 (Monterey) and later, both Apple Silicon and Intel.

Linux and Windows are not supported. The GPS path links against
CoreLocation through `objc2-core-location`, and the API surface
relies on macOS TCC (per-`.app`-bundle Location Services
authorization). Porting would require a separate location backend.

## Features

```bash
gmaps init                                    # Create or update settings (interactive wizard)
gmaps config                                  # Show current settings (API key masked)
gmaps places add <name> <lat,lng|address>     # Save a favorite place
gmaps places add <name> -H                    # Save current GPS location as a place
gmaps places list / remove                    # List or remove saved places
gmaps whereami                                # Print GPS location + address + place name
gmaps geocode <address>                       # Address → coordinates
gmaps reverse <lat,lng>                       # Coordinates → address
gmaps nearby <keyword> [-H] [--radius <m>]    # Search nearby places (-H = GPS, @name = saved place)
gmaps route <origin> <destination>            # Compute a route
```

All commands accept `--json` for piping. Set `DEBUG=1` to log requests.

## Development setup

This project ships a reproducible toolchain via **Nix Flakes + direnv**: Rust, `cargo-bundle`,
and the macOS frameworks needed by CoreLocation are all wired up for you.

### Prerequisites

- macOS (Apple Silicon / Intel)
- [Nix](https://nixos.org/download.html) (≥ 2.18, with flakes enabled)
- [direnv](https://direnv.net/) (optional but strongly recommended)

#### If you don't have Nix

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

#### If you don't have direnv

```bash
brew install direnv
eval "$(direnv hook zsh)"   # add to ~/.zshrc
```

### Setup

```bash
git clone https://github.com/YutaSugimura/gmaps-cli.git
cd gmaps-cli

direnv allow            # auto-load the flake
# or, every shell:
nix develop
```

When direnv is active, the following tools are added to `PATH`:

| Tool                                           | Purpose                        |
| ---------------------------------------------- | ------------------------------ |
| `rustc` / `cargo` 1.95                         | Rust toolchain                 |
| `rust-analyzer`                                | LSP server                     |
| `clippy` / `rustfmt`                           | Linter / formatter             |
| `cargo-bundle`                                 | `.app` bundle generation       |
| `cargo-watch` / `cargo-edit` / `cargo-nextest` | Development helpers            |
| `apple-sdk_15`                                 | macOS SDK (CoreLocation, etc.) |

## Build & run

### During development (`cargo run`)

```bash
cargo run -- init                    # wizard
cargo run -- nearby cafe --location 40.7580,-73.9855 --radius 500
cargo run -- route "Grand Central Terminal" "Times Square" --mode driving
```

> **Note**: `target/debug/gmaps` is a bare binary, so **GPS (`-H` / `--here`) does NOT work**.
> See the next section for `.app` bundling.

### Release build with `.app` bundle

```bash
./scripts/build.sh
```

The script:

1. Runs `cargo bundle --release` to produce `target/release/bundle/osx/gmaps.app`
2. Applies an **ad-hoc code signature** with `codesign --force --deep --sign -`
3. Validates `Info.plist` via `plutil -lint`
4. Prints follow-up install instructions

### Install as a global `gmaps` command

```bash
mkdir -p ~/.local/bin
ln -sf "$(pwd)/target/release/bundle/osx/gmaps.app/Contents/MacOS/gmaps" ~/.local/bin/gmaps

# add to ~/.zshrc if needed:
export PATH="$HOME/.local/bin:$PATH"
```

This routes `gmaps` through the `.app` bundle so CoreLocation authorization works.

### Plain binary (no GPS)

For scenarios where GPS isn't needed:

```bash
cargo build --release      # produces target/release/gmaps
```

## Google Cloud setup

Enable these three APIs in [Google Cloud Console](https://console.cloud.google.com/):

| API              | Used by                                                       |
| ---------------- | ------------------------------------------------------------- |
| Geocoding API    | `gmaps geocode` / `gmaps reverse` and address auto-resolution |
| Places API (New) | `gmaps nearby`                                                |
| Routes API       | `gmaps route`                                                 |

> Cloud Console lists older variants (Places API, Directions API). This tool uses the
> **new** ones (Places API (New), Routes API).

### API key restrictions (recommended)

1. **APIs & Services** → **Credentials** → **Create credentials** → **API key**
2. Edit the new key and restrict it to the three APIs above

### Budget alerts and quotas (strongly recommended)

- **Billing** → **Budgets & alerts**: set a low monthly cap (e.g., $1–$5)
- Cap each API at a sensible per-day quota (e.g., 500 requests/day)

### Initial setup

```bash
gmaps init
```

The wizard collects: API key, default location source (default / gps / manual), language,
and region; then verifies the key against the Geocoding API. Settings are written to
`~/.config/gmaps/config.yaml` with mode 0600.

## Usage examples

### Nearby search

```bash
# Keyword search
gmaps nearby cafe --radius 500 --limit 5

# Type filter (no keyword)
gmaps nearby --type restaurant --radius 1000
gmaps nearby --type convenience_store

# Explicit center
gmaps nearby pizza --location 40.7580,-73.9855
gmaps nearby pizza --location "Times Square"

# Use GPS (.app required)
gmaps nearby pizza -H

# Open now
gmaps nearby cafe --open-now
```

### Routing

```bash
gmaps route "Grand Central Terminal" "Times Square"                          # driving (default)
gmaps route "Grand Central Terminal" "Times Square" --mode walking --steps   # walking + step-by-step
gmaps route "Grand Central Terminal" "Brooklyn Bridge" --waypoints "Penn Station"
gmaps route "Grand Central Terminal" "Times Square" --depart 2026-04-30T18:00:00-04:00
```

> **Transit availability**: `--mode transit` is region-limited by Google.
> Coverage is good in much of the US and EU, while many other countries
> return no results — check Google's transit-coverage docs for your area.

### Geocoding

```bash
gmaps geocode "Statue of Liberty"
gmaps reverse 40.7580,-73.9855
```

### Saved places

Frequently-used locations live in `places.yaml`, separately from `config.yaml`,
and can be referenced with `@name`.

```bash
# Add by lat,lng or by address (auto-geocoded)
gmaps places add home 40.7484,-73.9857
gmaps places add office "Grand Central Terminal" --note "HQ"

# Capture current GPS location (.app required)
gmaps places add here -H

# List or remove
gmaps places list
gmaps places remove office

# Reference via @name
gmaps nearby cafe --location @home --radius 500
gmaps route @home @office --mode driving
gmaps route @home "Brooklyn Bridge" --waypoints "@office|Times Square"
```

### Settings

All edits go through `gmaps init`. Re-running fills each prompt with the current value
(press Enter to keep, type to override). There's no separate `config set` command.

```bash
gmaps init                                     # initial setup or update
gmaps config                                   # show current settings (API key masked)
```

### Center resolution priority

```
1. --location <lat,lng | address | @name>     # explicit (highest priority)
2. --here / -H                                  # GPS (CoreLocation, requires .app)
3. config.location_provider:
   - "gps":     try GPS, fall back to default_place
   - "default": use default_place
   - "manual":  --location is required (otherwise error)
```

**GPS authorization**: Allow `gmaps` under System Settings → Privacy & Security →
Location Services. The first run with `-H` will prompt you.

## Configuration files

```yaml
# ~/.config/gmaps/config.yaml (mode 0600)
api_key: "AIza..."
default_place: "home" # references places.yaml
language: "en"
region: "US"
location_provider: "default" # default | gps | manual
```

```yaml
# ~/.config/gmaps/places.yaml (mode 0600)
places:
  - name: home
    lat: 40.7484
    lng: -73.9857
  - name: office
    lat: 40.7527
    lng: -73.9772
    note: HQ office
```

## Cost overview

Google Maps Platform moved to per-SKU monthly free tiers in March 2025:

| API          | Free tier (Essentials) | Cost above free (per 1k requests) |
| ------------ | ---------------------- | --------------------------------- |
| Geocoding    | 10,000 / month         | $5                                |
| Routes       | 10,000 / month         | $5                                |
| Places (New) | 10,000 / month         | $32+ (depending on FieldMask)     |

For personal CLI use (a few dozen requests per day) you'll typically stay within the free
tier. Configure budget alerts and quotas anyway — accidents happen.

## Project layout

```
gmaps-cli/
├── flake.nix / flake.lock          # Nix dev shell
├── .envrc                          # direnv (use flake)
├── Cargo.toml                      # cargo-bundle metadata included
├── resources/
│   └── Info.plist.ext              # NSLocationWhenInUseUsageDescription
├── scripts/
│   └── build.sh                    # cargo bundle + ad-hoc signing
├── src/
│   ├── main.rs                     # clap entrypoint
│   ├── config.rs                   # YAML I/O
│   ├── http.rs                     # reqwest + error type
│   ├── format.rs                   # distance / duration / haversine
│   ├── wizard.rs                   # interactive setup
│   ├── api/
│   │   ├── geocoding.rs
│   │   ├── places.rs               # Places API (New)
│   │   └── routes.rs               # Routes API
│   ├── commands/
│   │   ├── config.rs
│   │   ├── geocode.rs
│   │   ├── nearby.rs
│   │   ├── places.rs
│   │   ├── route.rs
│   │   └── whereami.rs
│   └── location/
│       ├── mod.rs                  # LatLng / resolve_center
│       └── gps.rs                  # CoreLocation via objc2
└── README.md
```

## Troubleshooting

| Symptom                              | Fix                                                                           |
| ------------------------------------ | ----------------------------------------------------------------------------- |
| `PERMISSION_DENIED`                  | The API isn't enabled. Open the URL in the error and click "Enable".          |
| `REQUEST_DENIED` (API key not valid) | The API key is restricted in a way that excludes the API.                     |
| `OVER_QUERY_LIMIT`                   | Quota exceeded. Check Cloud Console.                                          |
| GPS times out                        | Bare binary outside an .app. Run `./scripts/build.sh` and use the `.app`.     |
| GPS dialog never appears             | Allow `gmaps` under System Settings → Privacy & Security → Location Services. |

## Debugging

```bash
DEBUG=1 gmaps nearby cafe --radius 500
```

Outputs request URL, FieldMask, and request/response JSON to stderr (API key masked).

## License

[MIT](LICENSE) © 2026 YutaSugimura

## Contributing

Contributions are welcome — bug reports, fixes, and small enhancements. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow, coding
conventions, and how to run the test/lint matrix locally.
