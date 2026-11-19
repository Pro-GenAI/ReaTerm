#!/usr/bin/env bash
# ReaTerm npm publisher: publish by default after validation.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${PACKAGE_DIR}/../.." && pwd)"
PACKAGE_JSON="${PACKAGE_DIR}/package.json"
REGISTRY="${NPM_REGISTRY:-https://registry.npmjs.org/}"
DIST_TAG="latest"
ACCESS="public"

usage() {
  cat <<'HELP'
Usage: bash src/typescript/scripts/npm_publish.sh [--dry-run | --publish] [--tag TAG] [--access public]

Package: reaterm (src/typescript).
By default, install/build/test, inspect the tarball, and publish.

  --dry-run           Validate without publishing
  --publish           Real publish after successful tests/pack verification (default)
  --tag TAG           npm dist-tag, default latest
  --access LEVEL      public only (unscoped package), default public
  -h, --help          Show this help

Environment: REATERM_BIN (optional trusted native binary), NPM_TOKEN
(optional auth for publishing), NPM_REGISTRY (HTTPS registry URL).
Only npm is supported: ReaTerm does not commit npm/pnpm lockfiles.
The npm package is a JS wrapper; consumers must separately install Rust CLI.
HELP
}

die() {
  echo "error: $*" >&2
  exit 1
}

dry=0
publish=0
while (($#)); do
  case "$1" in
    --dry-run) dry=1; shift ;;
    --publish) publish=1; shift ;;
    --tag)
      (($# >= 2)) || die "--tag requires a value"
      DIST_TAG="$2"; shift 2 ;;
    --access)
      (($# >= 2)) || die "--access requires public"
      ACCESS="$2"; shift 2 ;;
    -h|--help)
      (($# == 1)) || die "--help must be used alone"
      usage; exit 0 ;;
    *) die "unknown argument: $1 (use --help)" ;;
  esac
done
((dry == 0 || publish == 0)) || die "--dry-run and --publish conflict"
PUBLISH=$((1 - dry))
[[ "$ACCESS" == "public" ]] || die "unscoped npm packages must use --access public"
[[ "$DIST_TAG" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || die "invalid --tag"
[[ -z "${PACKAGE_MANAGER:-}" || "${PACKAGE_MANAGER}" == "npm" ]] || die "only npm is supported (no pnpm lockfile)"
[[ -f "$PACKAGE_JSON" ]] || die "missing $PACKAGE_JSON"

# Resolve a caller-supplied relative binary against the original working
# directory, before switching to src/typescript.
if [[ -n "${REATERM_BIN:-}" && "$REATERM_BIN" != /* ]]; then
  REATERM_BIN="$(cd "$(dirname "$REATERM_BIN")" && pwd)/$(basename "$REATERM_BIN")"
fi

# Never expose a publishing credential to npm install scripts, build tools,
# test processes, or npm pack. Restore it only as a private config for publish.
AUTH_TOKEN="${NPM_TOKEN:-}"
unset NPM_TOKEN

for program in node npm; do
  command -v "$program" >/dev/null 2>&1 || die "$program is required"
done

cd "$PACKAGE_DIR"
PACKAGE_NAME="$(node -p 'require("./package.json").name')"
PACKAGE_VERSION="$(node -p 'require("./package.json").version')"
[[ "$PACKAGE_NAME" == "reaterm" ]] || die "unexpected npm name: $PACKAGE_NAME"
[[ -n "$PACKAGE_VERSION" && "$PACKAGE_VERSION" != "undefined" ]] || die "missing npm version"
[[ -f "$REPO_ROOT/src/rust/Cargo.toml" ]] || die "not a ReaTerm checkout"

if ((PUBLISH)); then
  # Reject HTTP and URLs with embedded authentication.
  AUTH_KEY="$(node -e '
    const u = new URL(process.argv[1]);
    if (u.protocol !== "https:" || u.username || u.password || u.search || u.hash) process.exit(2);
    process.stdout.write("//" + u.host + u.pathname.replace(/\/?$/, "/") + ":_authToken");
  ' "$REGISTRY")" || die "publishing requires a clean HTTPS NPM_REGISTRY"
  echo "Publishing requested: $PACKAGE_NAME@$PACKAGE_VERSION to $REGISTRY"
else
  echo "Dry-run validation: $PACKAGE_NAME@$PACKAGE_VERSION (no auth or publishing)"
fi

# The native CLI must exist or all integration tests would silently skip.
NATIVE_BIN="${REATERM_BIN:-$REPO_ROOT/src/rust/target/release/reaterm}"
if [[ -z "${REATERM_BIN:-}" ]]; then
  command -v cargo >/dev/null 2>&1 || die "cargo required to build native CLI"
  cargo build --release --manifest-path "$REPO_ROOT/src/rust/Cargo.toml"
fi
[[ -x "$NATIVE_BIN" ]] || die "native binary missing or not executable: $NATIVE_BIN"
export REATERM_BIN="$NATIVE_BIN"
export REATERM_DECISION_MODEL=""

echo "Installing dependencies without creating package-lock.json"
npm install --no-package-lock --no-audit --no-fund
echo "Building adapter and running integration tests"
npm run check

umask 077
TEMP_DIR="$(mktemp -d)" || die "cannot create temporary directory"
cleanup() { rm -rf -- "$TEMP_DIR"; }
trap cleanup EXIT

echo "Checking publish tarball contents"
npm pack --dry-run --json --ignore-scripts > "$TEMP_DIR/pack.json"
node - "$TEMP_DIR/pack.json" "$PACKAGE_NAME" "$PACKAGE_VERSION" <<'NODE'
const fs = require("node:fs");
const [manifest, name, version] = process.argv.slice(2);
const packs = JSON.parse(fs.readFileSync(manifest, "utf8"));
if (packs.length !== 1 || packs[0].name !== name || packs[0].version !== version)
  throw Error("npm pack metadata disagrees with package.json");
const files = new Set(packs[0].files.map(f => f.path));
for (const required of ["dist/src/index.js", "dist/src/index.d.ts", "README.md", "package.json"]) {
  if (!files.has(required)) throw Error("Missing expected npm file: " + required);
}
for (const name of files) {
  if (name.startsWith("node_modules/") || /(^|\/)(\.env|\.npmrc)(\.|$)/.test(name))
    throw Error("Unsafe file in npm tarball: " + name);
}
console.log("Verified packed files:", [...files].sort().join(", "));
NODE

if ((!PUBLISH)); then
  echo "Dry-run passed. No login or publish was attempted."
  exit 0
fi

if [[ -n "$AUTH_TOKEN" ]]; then
  # Avoid writing secrets anywhere under the repository.
  printf 'registry=%s\n%s=%s\n' "$REGISTRY" "$AUTH_KEY" "$AUTH_TOKEN" > "$TEMP_DIR/.npmrc"
  export NPM_CONFIG_USERCONFIG="$TEMP_DIR/.npmrc"
fi
unset AUTH_TOKEN
echo "Checking npm registry authentication"
npm whoami --registry "$REGISTRY" >/dev/null || die "npm auth failed; login or set NPM_TOKEN"
npm publish --registry "$REGISTRY" --tag "$DIST_TAG" --access "$ACCESS"
echo "Published $PACKAGE_NAME@$PACKAGE_VERSION"
