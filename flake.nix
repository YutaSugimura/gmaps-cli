{
  description = "gmaps-cli — Google Maps Platform CLI (macOS only)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        isDarwin = pkgs.stdenv.isDarwin;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
          targets = [
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
          ];
        };

        commonPackages = with pkgs; [
          rustToolchain
          cargo-bundle
          cargo-watch
          cargo-edit
          cargo-nextest
          pkg-config
        ];

        darwinPackages = with pkgs; [
          # Required for linking against macOS frameworks (CoreLocation, etc.)
          apple-sdk_15
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = commonPackages
            ++ pkgs.lib.optionals isDarwin darwinPackages;

          shellHook = ''
            # ── gmaps wrapper: drop into .direnv/bin and add to PATH ──
            # Disappears from PATH when leaving the shell, so no global
            # install is needed.
            PROJECT_ROOT="$PWD"
            WRAP_DIR="$PROJECT_ROOT/.direnv/bin"
            mkdir -p "$WRAP_DIR"

            # gmaps: build gmaps.app on demand if missing, then exec it.
            cat > "$WRAP_DIR/gmaps" <<'WRAPPER'
            #!/usr/bin/env bash
            set -e
            PROJECT_ROOT="__PROJECT_ROOT__"
            APP_BIN="$PROJECT_ROOT/target/release/bundle/osx/gmaps.app/Contents/MacOS/gmaps"
            if [ ! -x "$APP_BIN" ]; then
              echo "▶ Building gmaps.app (first run only, ~1 minute)..." >&2
              "$PROJECT_ROOT/scripts/build.sh" >&2
            fi
            exec "$APP_BIN" "$@"
            WRAPPER
            sed -i.bak "s|__PROJECT_ROOT__|$PROJECT_ROOT|" "$WRAP_DIR/gmaps" \
              && rm -f "$WRAP_DIR/gmaps.bak"
            chmod +x "$WRAP_DIR/gmaps"

            # gmaps-rebuild: force a rebuild (run manually after source changes).
            cat > "$WRAP_DIR/gmaps-rebuild" <<'WRAPPER'
            #!/usr/bin/env bash
            set -e
            PROJECT_ROOT="__PROJECT_ROOT__"
            exec "$PROJECT_ROOT/scripts/build.sh"
            WRAPPER
            sed -i.bak "s|__PROJECT_ROOT__|$PROJECT_ROOT|" "$WRAP_DIR/gmaps-rebuild" \
              && rm -f "$WRAP_DIR/gmaps-rebuild.bak"
            chmod +x "$WRAP_DIR/gmaps-rebuild"

            export PATH="$WRAP_DIR:$PATH"

            # ── Welcome message ──
            echo "──────────────────────────────────────"
            echo " gmaps-cli dev shell"
            echo "──────────────────────────────────────"
            echo " rustc:  $(rustc --version)"
            echo " cargo:  $(cargo --version)"
            echo " target: ${system}"
            if [ -x "$PROJECT_ROOT/target/release/bundle/osx/gmaps.app/Contents/MacOS/gmaps" ]; then
              echo " gmaps:  ✓ built (run via 'gmaps')"
            else
              echo " gmaps:  not built (first 'gmaps ...' call builds it)"
            fi
            echo "         rebuild with 'gmaps-rebuild'"
            echo "──────────────────────────────────────"
          '';

          # Some crates (e.g. ring) may need this to find the std source.
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
