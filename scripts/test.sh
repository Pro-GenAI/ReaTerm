#!/usr/bin/env bash
# Compatibility runner: the integrated Make verification is authoritative.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
case "$(uname -s)" in
  Linux|Darwin) ;;
  *) echo "ReaTerm enforcement tests require Linux or macOS" >&2; exit 2 ;;
esac
if [[ "$#" -gt 1 ]] || { [[ "$#" -eq 1 ]] && [[ "$1" != "--stress" ]]; }; then
  echo "Usage: bash scripts/test.sh [--stress]" >&2
  exit 2
fi

make -C "$ROOT" verify

if [[ "$#" -eq 1 ]]; then
  echo "[optional] Slow 10001-entry directory stress test"
  cargo test --manifest-path "$ROOT/src/rust/Cargo.toml" --test resource_limits -- --ignored
fi

echo "ReaTerm regression suite completed successfully"
