# ReaTerm testing and security regression guide

The tests below validate supported read-only commands, fail-closed behavior,
filesystem isolation, syscall protections, language adapters, and resource
limits. No test deliberately modifies files outside its own temporary
fixtures.

## Prerequisites and safety

Use Linux x86-64/ARM64 with Landlock ABI v3+ and seccomp or macOS Intel/
Apple Silicon with Seatbelt, plus Rust/Cargo, Python 3.11+, Node.js 20+
and npm. The former four-platform CI matrix passed before it was disabled.
Both existing CI workflows are currently disabled.
Do **not** run the test suite as root.

Tests create files in temporary directories and attempt malicious operations
only against those disposable fixtures. A vulnerable binary might still
alter a test fixture. The test runner itself uses Cargo and npm, which
**write developer build artifacts as expected**; only the ReaTerm binary
is intended to remain read-only.

## Run the full regression suite

From the repository root:

    bash scripts/test.sh

Optional slower directory-entry stress test (creates 10,001 temporary files):

    bash scripts/test.sh --stress

The runner checks formatting and Clippy, runs Rust unit/integration tests,
builds a release executable, tests the Python adapter against that binary,
and installs/builds/tests the TypeScript adapter. It exits nonzero if any
required test fails. It deliberately unsets REATERM_DECISION_MODEL to
prevent accidental external model requests in routine tests.

Run each language independently:

    cargo fmt --manifest-path src/rust/Cargo.toml --all -- --check
    cargo clippy --manifest-path src/rust/Cargo.toml --all-targets -- -D warnings
    cargo test --manifest-path src/rust/Cargo.toml --all -- --nocapture
    cargo build --manifest-path src/rust/Cargo.toml --release

    export REATERM_BIN="$PWD/src/rust/target/release/reaterm"
    python3 -m unittest discover -s src/python/tests -p 'test_*.py' -v

    cd src/typescript
    npm install --no-audit --no-fund
    npm run check

To run ignored resource stress tests without the wrapper:

    cargo test --manifest-path src/rust/Cargo.toml \
      --test resource_limits -- --ignored

## Coverage matrix

| Test suite | Coverage |
|---|---|
| Rust unit tests | Path normalization, stdout bounds, unknown commands; seccomp chmod/fchmod/chown/fchown/xattr/timestamp denial |
| Rust tests/readonly.rs | Destructive command matrix, unchanged file contents/modes/directory entries |
| Rust tests/operations.rs | Correct outputs for every supported command, sorted/hidden listings, nested paths, binary data, unusual filenames, content/mode integrity |
| Rust tests/paths_and_policy.rs | Invalid flags and arity, absolute/parent traversal, escaping symlinks, sockets and directories, malformed operands |
| Rust tests/resource_limits.rs | Exactly 1 MiB stdout vs overflow, oversized files, invalid line counts |
| Rust tests/resource_limits.rs ignored | 10,001 directory-entry stress test (opt-in) |
| Python test_convenience.py | Package-level run import, `ls -al` argv splitting, quotes, shell-syntax rejection, native denial |
| TypeScript convenience.test.ts | ESM top-level export, shell-syntax rejection, native `ls -al` and quoted filenames |
| Python test_tools.py | OpenAI Chat Completions/Responses and Anthropic tool schemas, strict arguments, literal-argv execution, native denial and optional disposable native integration |
| Python test_client.py and test_security.py | No shell interpolation, error and timeout handling, native bridge, model veto and missing credentials, fake SDK and environment precedence |
| TypeScript client.test.ts and security.test.ts | Real native execution, denied mutations and path escape, mode preservation, missing binary, mock local OpenAI endpoint and veto |
| GitHub Actions smoke step | Release binary reads, rejects destructive commands/unsupported flags/root escapes |

## Core security assertions

1. Only pwd, ls, cat, head, tail, wc, grep, stat and supported arguments
   succeed. rm, mv, cp, touch, chmod, chown, chgrp, truncate, tee, shell
   interpreters, unsafe Git/find/sed operations and unknown commands fail.
2. Absolute paths and parent traversal fail. A symlink must not allow reading
   outside the root. Inspecting link metadata is allowed; following an
   external link is not. Special-file content reads fail.
3. One MiB of output is allowed, but one byte more is denied. Files over
   10 MiB and excessive line counts are refused.
4. The seccomp filter denies chmod, fchmod, chown, fchown, setxattr and
   utimensat on disposable fixtures, even when the file is already open.
5. Python and TypeScript always pass argv directly to the Rust binary;
   shell metacharacters are literal filenames, never shell programs.
6. Optional OpenAI-compatible decisions can veto but never grant commands.
   Provider tool schemas cannot execute automatically or choose the executable
   or root; `execute_reaterm_tool` validates tool arguments and dispatches argv.
   All model test calls use a fake SDK or a 127.0.0.1 test server, never
   external paid providers.
7. Test files retain the same contents, permission bits, and modification
   time after read-only operations. Access time (atime) is not compared,
   because ordinary reads may update it.

## Limits of the verification

Passing tests is not a formal proof against all kernel vulnerabilities
or syscall variants. The parent shell, other processes, and file descriptors
inherited from the parent are not controlled by ReaTerm. For example, the
parent shell can still create or truncate a file using output redirection
around ReaTerm, before the binary launches. ReaTerm is also not a
confidentiality boundary: the caller chooses the readable root.

CI runs both ARM64 Linux and ARM64 macOS as well as their x86-64
counterparts. ARM64 Linux VMs do not establish that your particular
Raspberry Pi's kernel has Landlock enabled: run the suite on the Pi.

## Future contributions

For every new command or flag, add positive output tests, negative
side-effect tests, path/traversal checks and bounded-resource checks.
For every new sandbox mechanism, verify its rejection in an isolated
fixture or dedicated test thread. Update docs/TESTING.md,
docs/DEVELOPMENT.md, docs/THREAT_MODEL.md and AGENTS.md when applicable.

 
## Exclusive informational CLI flags

Rust tests/cli_flags.rs and the parser unit tests verify each of
--help/-h, --version/-V, --list-commands, --limits, --license, and
--about independently. Each informational flag must appear **alone**.
Mixing one with a valid read command, destructive command, another
informational flag, --root or -C yields exit code 2, no stdout, and
a specific diagnostic before dispatch.

Tests check both flag positions (e.g. reaterm --help cat FILE and
reaterm cat FILE --help), invalid/missing roots, and command operands
that would otherwise be read. Readable canary file contents, permissions,
modification times and directory entries are asserted unchanged.
Execution root forms --root DIR, --root=DIR and -C DIR are also tested.

These tests run in ordinary CI (cargo test). The executable's own
information-only flags work without opening a root or requiring Landlock;
actual file operations still mandate Landlock and seccomp on Linux.

 
## Static website regression tests

The standalone site in docs/website includes dependency-free Python
checks for internal links/assets, site metadata, accessibility landmarks,
tab/panel relationships, package source links and the informational
PyPI/npm registry links, sitemap, offline asset independence, and local
HTTP delivery. Run:

    python -m unittest discover -s docs/website -p "test_*.py" -v

The CI workflows are currently disabled, so run these tests locally.
This test suite validates static files, not a deployed domain, production
DNS, TLS, or external package registry publication.

 
## Unified Make commands and fail-fast validation

The primary integrated developer workflow is:

    make setup
    make test

The first command installs/builds development prerequisites in Rust, Python,
and TypeScript; the second runs their tests (plus the website checks) in
sequence and assumes setup has succeeded. Each command stops on the first
failure. Individual language targets are
under src/rust/, src/python/, src/typescript/, and docs/website/.

A dedicated standard-library suite uses temporary fake Makefiles to check
successful order, each language's setup/test failure, and `make -j8`:

    python3 -m unittest discover -s scripts -p 'test_make*.py' -v

The disabled cross-platform CI workflow previously ran these orchestration
tests and a real end-to-end verification on Linux. The legacy
`bash scripts/test.sh [--stress]` entry point remains available.

 
## Native platform verification

The disabled GitHub Actions matrix previously ran the full suite on Linux x86-64
(`ubuntu-latest`), Linux ARM64 (`ubuntu-24.04-arm`), macOS Intel
(`macos-15-intel`) and macOS Apple Silicon (`macos-15`).
All Rust native CLI and adapter integration tests execute against their
platform's OS sandbox. The macOS direct-Seatbelt test creates disposable
fixtures and verifies chmod/chown/file-creation/deletion refusal in a
separate child process. Only Linux runs the seccomp/Landlock unit tests.
Test support on your real Raspberry Pi with `make setup` and `make test`,
not just a cross-compiled binary.


## Windows x86-64 regression tests

Windows uses a dedicated PowerShell fail-fast runner:

    .\scripts\verify-windows.ps1 -Mode setup
    .\scripts\verify-windows.ps1 -Mode test

These commands set up Rust, Python, TypeScript and the website, then run
their tests. Each command stops on the first failure.
If GNU Make is installed on Windows, Make targets delegate to the same
PowerShell runner. Windows-specific tests create private disposable
fixtures and verify that normal read commands work, dangerous commands
are refused, path escapes fail, and actual Win32 low-integrity restricted-token write/delete attempts cannot
modify medium-integrity files with normal DACLs. The optional Windows
symlink fixture requires Developer Mode or symlink privileges.

The disabled .github/workflows/windows.yml.disabled targeted windows-2022 and
publishes a compiled x86-64 .exe artifact after a successful run.
The prior ci.yml.disabled is intentionally not re-enabled.

 
## Publishing workflow safety checks

Run `python3 -m unittest discover -s scripts -p 'test_publish*.py' -v`
before changing either publishing entry point. The tests verify the PyPI
release trigger, actual package names and versions, native security tests
before upload, distribution build and wheel installation, and perform
sandboxed npm script dry-runs and fake publications in disposable
directories. They check that setup/test/pack failures abort publication,
publish requires `--publish`, and temporary token config is removed.

No test performs an actual upload or requires npm/PyPI credentials.
The publication action depends on PyPI Trusted Publishing configuration,
which this offline suite cannot verify. The old CI workflows remain
disabled; this PyPI workflow is independently triggered by a **published**
GitHub release.
