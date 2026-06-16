# Installation

`gmaps` is distributed as a macOS `.app` bundle (CoreLocation requires
this for GPS authorization). The bundle is Developer ID signed and
notarized, so it launches without any Gatekeeper bypass.

> Apple Silicon only. Intel Macs build from source — see
> [From source](#from-source).

## With Homebrew (recommended)

```bash
brew tap YutaSugimura/tap
brew trust YutaSugimura/tap        # Homebrew 5.x requires trusting third-party taps once
brew install --cask gmaps
gmaps --version
```

Homebrew installs `gmaps.app` and symlinks the `gmaps` command onto your
`PATH` automatically — no manual symlink or `PATH` editing needed. Upgrade
with `brew upgrade --cask gmaps`; remove with `brew uninstall --cask gmaps`.

The first `gmaps -H` invocation triggers the Location Services
authorization dialog.

## From a prebuilt release

Grab the latest `.app` zip from the [Releases page](https://github.com/YutaSugimura/gmaps-cli/releases/latest).

```bash
# Download and unzip
curl -L -o gmaps.app.zip \
  "https://github.com/YutaSugimura/gmaps-cli/releases/latest/download/gmaps-0.1.3-macos-arm64.app.zip"
unzip gmaps.app.zip
mv gmaps.app /Applications/

# Signed with a Developer ID and notarized by Apple, so it launches without
# any Gatekeeper bypass. gmaps.app is a CLI helper (LSUIElement), so no
# window will appear.

# Symlink the binary onto your PATH
mkdir -p ~/.local/bin
ln -sf /Applications/gmaps.app/Contents/MacOS/gmaps ~/.local/bin/gmaps

# Ensure ~/.local/bin is on PATH (zsh)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc

gmaps --version
```

The first `gmaps -H` invocation will trigger the Location Services
authorization dialog.

> **Intel Macs**: prebuilt zips are arm64 only for now. Build from
> source — see [Development setup](README.md#development-setup).

### Verify download integrity

Each Release notes body includes a SHA256 line. Verify after download:

```bash
shasum -a 256 gmaps-0.1.3-macos-arm64.app.zip
```

## From source

See [Development setup](README.md#development-setup) in the README
for the Nix-based dev shell. Source builds are required for Intel
Macs and for the latest unreleased changes.

## First-time setup

After installing, run the interactive setup wizard:

```bash
gmaps init
```

It walks you through:

- **API key** — your Google Maps Platform key (masked input). Get one at the
  [Cloud Console](https://console.cloud.google.com/google/maps-apis/credentials)
  and enable **Geocoding API**, **Places API (New)**, and **Routes API**. The
  wizard verifies the key with a live request before saving.
- **Default location source** — one of:
  - **Fixed place** — a saved coordinate (e.g. `home`) used when you don't
    pass `--location`.
  - **GPS (CoreLocation)** — uses your Mac's location with `gmaps -H` /
    `--here`.
  - **Manual** — always require an explicit `--location`.
- **Language / region** — e.g. `ja` / `JP`.

Re-run `gmaps init` anytime to change settings (press Enter to keep an
existing value). Manage saved places later with `gmaps places ...`.

Settings live under `~/.config/gmaps/` (created with mode `0600`):

| File | Contents |
| --- | --- |
| `config.yaml` | API key, default location source, language/region |
| `places.yaml` | saved places |

The first `gmaps -H` invocation triggers the macOS Location Services
prompt; approve `gmaps` under **System Settings › Privacy & Security ›
Location Services**.

## Clean reinstall / uninstall

```bash
# 1. Remove the app and the `gmaps` command
brew uninstall --cask gmaps

# 2. Remove your config and saved places
rm -rf ~/.config/gmaps

# 3. (optional) Remove the tap
brew untap YutaSugimura/tap
```

To revoke the Location Services permission, toggle `gmaps` **off** under
**System Settings › Privacy & Security › Location Services**. macOS keeps
location authorization in a SIP-protected store that `tccutil` cannot reset
(`tccutil reset Location` fails for both a single app and globally), and the
permission survives an uninstall/reinstall since the bundle id is unchanged.

For a clean reinstall, run the steps above, then install again and
re-run setup:

```bash
brew install --cask gmaps
gmaps init
```

> Installed from a prebuilt zip instead of Homebrew? Replace step 1 with
> `rm -rf /Applications/gmaps.app ~/.local/bin/gmaps`.
