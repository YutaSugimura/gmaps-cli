# gmaps-cli (`gmaps`)

[![CI](https://github.com/YutaSugimura/gmaps-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/YutaSugimura/gmaps-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)](#supported-platforms)

A command-line interface for Google Maps Platform — nearby search, directions,
geocoding, and GPS-driven location lookups, all from your terminal.

> **OS**: macOS 12+ (Apple Silicon / Intel) &nbsp;•&nbsp; **Language**: Rust 1.95+
> &nbsp;•&nbsp; **Command**: `gmaps`

## Supported platforms

macOS only. The GPS path links against CoreLocation through
`objc2-core-location` and relies on macOS TCC (per-`.app`-bundle Location
Services authorization), so `gmaps` ships as a signed `.app` bundle. Linux
and Windows would need a separate location backend and are not supported.

Prebuilt releases are Apple Silicon only; Intel Macs build from source (see
[CONTRIBUTING.md](CONTRIBUTING.md#development-environment)).

## Install

```bash
brew tap YutaSugimura/tap
brew trust YutaSugimura/tap        # Homebrew 5.x: trust third-party taps once
brew install --cask gmaps
gmaps --version
```

The bundle is Developer ID signed and notarized, so it launches without any
Gatekeeper bypass, and Homebrew puts the `gmaps` command on your `PATH`
automatically.

See **[INSTALL.md](INSTALL.md)** for the prebuilt-zip path, building from
source, and clean reinstall / uninstall.

## Setup

Enable three APIs for your key in the
[Google Cloud Console](https://console.cloud.google.com/google/maps-apis/credentials)
— **Geocoding API**, **Places API (New)**, and **Routes API**. Cloud Console
also lists legacy variants (Places API, Directions API); pick the **new**
ones. Restricting the key to just these three, and setting a low billing
budget + per-API daily quota, is strongly recommended.

Then run the interactive wizard:

```bash
gmaps init
```

It collects your API key, default location source, and language/region, then
verifies the key with a live request before writing
`~/.config/gmaps/config.yaml` (mode 0600). Re-run anytime to change settings.
Full walkthrough: [INSTALL.md → First-time setup](INSTALL.md#first-time-setup).

## Commands

```bash
gmaps init                                    # Create or update settings (interactive wizard)
gmaps config                                  # Show current settings (API key masked)
gmaps places add <name> <lat,lng|address>     # Save a favorite place
gmaps places add <name> -H                    # Save current GPS location as a place
gmaps places list / remove                    # List or remove saved places
gmaps whereami                                # Print GPS location + address + place name
gmaps geocode <address>                       # Address → coordinates
gmaps reverse <lat,lng>                       # Coordinates → address
gmaps nearby <keyword> [-H] [--radius <m>] [--map]  # Search nearby places (-H = GPS, @name = saved place, --map = ASCII map)
gmaps route <origin> <destination>            # Compute a route
```

All commands accept `--json` for piping. Set `DEBUG=1` to log requests (see
[Debugging](#debugging)).

## Usage examples

### Nearby search

```bash
# Keyword search
gmaps nearby cafe --radius 500 --limit 5

# Type filter (no keyword)
gmaps nearby --type restaurant --radius 1000
gmaps nearby --type convenience_store

# Explicit center (lat,lng, address, or @saved-place)
gmaps nearby pizza --location 40.7580,-73.9855
gmaps nearby pizza --location "Times Square"
gmaps nearby cafe --location @home --radius 500

# Use GPS (.app required)
gmaps nearby pizza -H

# Open now
gmaps nearby cafe --open-now

# Render an ASCII map of results around the center (in addition to the table)
gmaps nearby cafe --radius 500 --map
```

`--map` draws a compass-oriented grid (N/S/E/W) with each result numbered to
match the table; the header notes the meters-per-cell scale, and any results
beyond the grid are listed as `off-map`. It is ignored with `--json`.

### Routing

```bash
gmaps route "Grand Central Terminal" "Times Square"                          # driving (default)
gmaps route "Grand Central Terminal" "Times Square" --mode walking --steps   # walking + step-by-step
gmaps route "Grand Central Terminal" "Brooklyn Bridge" --waypoints "Penn Station"
gmaps route "Grand Central Terminal" "Times Square" --depart 2026-04-30T18:00:00-04:00
gmaps route @home @office --mode driving                                     # saved places
```

> **Transit availability**: `--mode transit` is region-limited by Google.
> Coverage is good in much of the US and EU, while many other countries
> return no results — check Google's transit-coverage docs for your area.

### Geocoding

```bash
gmaps geocode "Statue of Liberty"     # address → coordinates
gmaps reverse 40.7580,-73.9855        # coordinates → address
```

### Saved places

Frequently-used locations live in `places.yaml`, separately from
`config.yaml`, and can be referenced with `@name` anywhere a location is
accepted.

```bash
# Add by lat,lng or by address (auto-geocoded)
gmaps places add home 40.7484,-73.9857
gmaps places add office "Grand Central Terminal" --note "HQ"

# Capture current GPS location (.app required)
gmaps places add here -H

# List or remove
gmaps places list
gmaps places remove office
```

### Center resolution priority

When a command needs a center point, it is resolved in this order:

```
1. --location <lat,lng | address | @name>     # explicit (highest priority)
2. --here / -H                                 # GPS (CoreLocation, requires .app)
3. config.location_provider:
   - "gps":     try GPS, fall back to default_place
   - "default": use default_place
   - "manual":  --location is required (otherwise error)
```

**GPS authorization**: the first run with `-H` prompts you. Allow `gmaps`
under System Settings → Privacy & Security → Location Services.

## Configuration files

Both files live under `~/.config/gmaps/` and are created with mode 0600. All
edits go through `gmaps init` (settings) and `gmaps places` (places) — there
is no separate `config set` command.

```yaml
# config.yaml
api_key: "AIza..."
default_place: "home" # references places.yaml
language: "en"
region: "US"
location_provider: "default" # default | gps | manual
```

```yaml
# places.yaml
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

For personal CLI use (a few dozen requests per day) you'll typically stay
within the free tier. Configure budget alerts and quotas anyway — accidents
happen.

## Troubleshooting

| Symptom                              | Fix                                                                                                            |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `gmaps: command not found`           | Homebrew installs handle PATH automatically. For a manual `.app` install, symlink it: `ln -sf /Applications/gmaps.app/Contents/MacOS/gmaps ~/.local/bin/gmaps` (and ensure `~/.local/bin` is on PATH). |
| `PERMISSION_DENIED`                  | The API isn't enabled. Open the URL in the error and click "Enable".                                           |
| `REQUEST_DENIED` (API key not valid) | The API key is restricted in a way that excludes the API.                                                      |
| `OVER_QUERY_LIMIT`                   | Quota exceeded. Check Cloud Console.                                                                           |
| GPS times out / no prompt            | Enable `gmaps` under System Settings → Privacy & Security → Location Services. Directly-launched CLIs often get no prompt; toggle it on manually (`tccutil` cannot reset Location). From source, the binary must be the `.app` (run `./scripts/build.sh`). |

## Debugging

```bash
DEBUG=1 gmaps nearby cafe --radius 500
```

Outputs the request URL, FieldMask, and request/response JSON to stderr (API
key masked).

## Contributing

Contributions are welcome — bug reports, fixes, and small enhancements. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, build flow,
coding conventions, and the test/lint matrix.

## License

[MIT](LICENSE) © 2026 YutaSugimura
