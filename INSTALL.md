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
  "https://github.com/YutaSugimura/gmaps-cli/releases/latest/download/gmaps-0.1.1-macos-arm64.app.zip"
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
shasum -a 256 gmaps-0.1.1-macos-arm64.app.zip
```

## From source

See [Development setup](README.md#development-setup) in the README
for the Nix-based dev shell. Source builds are required for Intel
Macs and for the latest unreleased changes.
