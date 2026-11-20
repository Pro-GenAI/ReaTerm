#!/usr/bin/env bash
# Update the shared release version. With no argument, increment the patch.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python3 -c "$(cat <<'PY'
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
if len(sys.argv) > 3:
    sys.exit("usage: update-versions.sh [new-version]")

paths = [root / p for p in (
    "src/python/pyproject.toml",
    "src/rust/Cargo.toml",
    "src/rust/Cargo.lock",
    "src/typescript/package.json",
)]
patterns = [
    r'(?m)^version = "([^"\n]+)"',
    r'(?m)^version = "([^"\n]+)"',
    r'(?m)(?<=\nname = "reaterm"\n)version = "([^"\n]+)"',
    r'(?m)^  "version": "([^"\n]+)"',
]
indexes = [0, 1, 2, 3]
# A lockfile is optional: npm install is used when one is not committed.
# If a checkout has a lockfile, keep both of its root versions in sync.
npm_lock = root / "src/typescript/package-lock.json"
if npm_lock.is_file():
    paths.append(npm_lock)
    patterns.extend((
        r'(?m)^  "version": "([^"\n]+)"',
        r'(?m)(?<=\n    "": {\n      "name": "reaterm",\n)      "version": "([^"\n]+)"',
    ))
    indexes.extend((4, 4))
texts = [p.read_text(encoding="utf-8") for p in paths]
matches = []
for index, pattern in zip(indexes, patterns):
    found = list(re.finditer(pattern, texts[index]))
    if len(found) != 1:
        sys.exit(f"error: expected exactly one project version in {paths[index]}")
    matches.append(found[0].group(1))

semver = re.compile(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z')
if any(not semver.fullmatch(v) for v in matches):
    sys.exit("error: existing versions must use numeric major.minor.patch format")
if json.loads(texts[3])["name"] != "reaterm" or (npm_lock.is_file() and json.loads(texts[4])["packages"][""]["name"] != "reaterm"):
    sys.exit("error: unexpected npm package name")
current = max(matches, key=lambda v: tuple(map(int, v.split('.'))))
if len(set(matches)) != 1:
    print(f"warning: versions differ; using {current} as baseline", file=sys.stderr)
if len(sys.argv) == 3:
    new = sys.argv[2]
    if not semver.fullmatch(new):
        sys.exit("error: version must use numeric major.minor.patch format")
    if tuple(map(int, new.split('.'))) <= tuple(map(int, current.split('.'))):
        try:
            answer = input(f"warning: {new} is not greater than {current}. Continue anyway? [y/N] ")
        except EOFError:
            answer = ""
        if answer.lower() != "y":
            sys.exit("aborted")
else:
    major, minor, patch = map(int, current.split('.'))
    new = f"{major}.{minor}.{patch + 1}"

for index, pattern in zip(indexes, patterns):
    texts[index] = re.sub(pattern, lambda m: m.group(0).replace(m.group(1), new, 1), texts[index], count=1)
for path, text in zip(paths, texts):
    path.write_text(text, encoding="utf-8")
print(f"Version updated: {current} -> {new}")
for path in paths:
    print(f"  {path}")
PY
)" "$ROOT" "$@"
