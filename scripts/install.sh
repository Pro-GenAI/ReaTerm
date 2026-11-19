#!/usr/bin/env bash
# Installs the Rust binary to Cargo's local executable directory.
# Installer writes build artifacts; the ReaTerm runtime is read-only.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "$#" -ne 0 ]]; then
  echo "Usage: bash scripts/install.sh" >&2
  exit 2
fi
case "$(uname -s):$(uname -m)" in
  Linux:x86_64|Linux:aarch64|Darwin:x86_64|Darwin:arm64) ;;
  *)
    echo "Unsupported platform: $(uname -s) $(uname -m). Supported: Linux x86_64/aarch64 and macOS x86_64/arm64" >&2
    exit 2
    ;;
esac
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required; install Rust from https://rustup.rs/" >&2
  exit 2
fi

cargo install --path "$ROOT/src/rust" --force
INSTALL_ROOT="$(printenv CARGO_INSTALL_ROOT || printenv CARGO_HOME || printf '%s' "$HOME/.cargo")"
INSTALLED="$INSTALL_ROOT/bin/reaterm"

echo "Installed CLI at $INSTALLED"
"$INSTALLED" --version
if ! command -v reaterm >/dev/null 2>&1; then
  echo "Add the Cargo bin directory to PATH:"
  echo "  export PATH=\"$INSTALL_ROOT/bin:\$PATH\""
fi
echo "Run: reaterm --help"
echo "Run: reaterm ls -la"
