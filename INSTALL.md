# Installation

`gmaps` is distributed as a macOS `.app` bundle (CoreLocation requires
this for GPS authorization). Two paths are supported.

## From a prebuilt release (recommended)

Grab the latest `.app` zip from the [Releases page](https://github.com/YutaSugimura/gmaps-cli/releases/latest).

```bash
# Download and unzip
curl -L -o gmaps.app.zip \
  "https://github.com/YutaSugimura/gmaps-cli/releases/latest/download/gmaps-0.1.0-macos-arm64.app.zip"
unzip gmaps.app.zip
mv gmaps.app /Applications/

# First launch: right-click /Applications/gmaps.app → "Open" to bypass Gatekeeper.
# (Ad-hoc signed; this is a one-time confirmation.)
# gmaps.app is a CLI helper (LSUIElement), so no window will appear.

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
shasum -a 256 gmaps-0.1.0-macos-arm64.app.zip
```

## From source

See [Development setup](README.md#development-setup) in the README
for the Nix-based dev shell. Source builds are required for Intel
Macs and for the latest unreleased changes.
