# ReaTerm Development Guide

This is the central reference for contributors. Setup, testing, build
orchestration, platform verification, publishing and deployment belong in
developer documentation—not in README landing pages. Every README is a
**user-facing product overview** with benefits and short usage examples.

## Source installation and prerequisites

Install Rust/Cargo, Python 3.11+, Node.js 20+ and your platform's required
native sandbox facilities. Linux requires Landlock ABI v3 (approximately kernel
6.2+) and seccomp. ReaTerm fails closed when required sandbox facilities are
missing. The wrapper packages do not contain the native executable.

~~~bash
git clone https://github.com/Pro-GenAI/ReaTerm.git
cd ReaTerm
cargo install --locked --path src/rust
python -m pip install -e ./src/python
# Optional TypeScript/Node adapter:
(cd src/typescript && npm install --no-audit --no-fund && npm run build)
~~~

Ensure Cargo's binary directory is on `PATH` or set `REATERM_BIN` to the
installed native executable. The repository's current native dependency lock
uses the patched cap-std and cap-primitives 3.4.6 releases. Package
publication to PyPI/npm has not been verified.

## Developer quick start

From the repository root:

~~~bash
make setup           # Build Rust, provision Python, install npm dependencies
make test            # Test Rust, Python, TypeScript and the static website
make verify          # Sequential, fail-fast setup and verification
~~~

These targets build dependencies and create disposable test fixtures; only
the ReaTerm runtime is intended to be read-only. Native integration tests
require supported OS enforcement. Do not run sandbox tests as root.

### MCP adapter development

The Python and TypeScript packages each provide a local stdio MCP
adapter; neither modifies the native command set. The `mcp` extra uses the official MCP Python SDK
v2 (`MCPServer` API). To exercise the real protocol, install it in a
virtual environment alongside the Python package and build the native
binary separately:

~~~bash
python -m pip install -e './src/python[mcp]'
reaterm-mcp --root /absolute/path/to/test-project
~~~

The host launches this process and communicates on stdin/stdout. A
standalone launch will wait for MCP requests. `--root` is required and
`--binary` or `REATERM_BIN` can select the trusted native executable.
The fast Python unittest suite retains isolated handler mocks. The
authoritative MCP protocol tests use `pytest-mcptest==0.1.1` (MCP v2),
which starts the actual Python stdio server and drives it with a real client.
`make verify` installs the Python `[mcp-test]` extra and runs those tests
after the unittests; the Windows PowerShell runner does the same. The
TypeScript MCP tests use `@modelcontextprotocol/client` v2, with linked
in-memory transports for invalid schemas and a child-process stdio
transport for handshake, native access, and the offline model-veto
regression. No JSON-RPC framing is simulated by hand; no external model
provider is contacted. The native Rust binary must still be installed
separately. Node.js MCP clients can invoke
`reaterm-mcp --root /absolute/path/to/test-project` from the package bin;
`--binary` optionally selects the trusted native executable.

### Targeted checks

~~~bash
cargo fmt --manifest-path src/rust/Cargo.toml --all -- --check
cargo clippy --manifest-path src/rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src/rust/Cargo.toml
cargo test --manifest-path src/rust/Cargo.toml --test security_matrix
python3 -m unittest discover -s src/python/tests -p 'test_*.py' -v
python3 -m unittest discover -s src/python/tests -p 'test_security_matrix.py' -v
# After pip install -e './src/python[mcp-test]' and native CLI setup:
REATERM_BIN="$PWD/src/rust/target/release/reaterm" python3 -m pytest -q src/python/mcp_tests
cd src/typescript && npm install && npm run check
# Return to the repo root:
python3 -m unittest discover -s docs/website -p 'test_*.py' -v
~~~

On Windows, use the PowerShell runner from the repository root:

~~~powershell
.\scripts\verify-windows.ps1 -Mode setup
.\scripts\verify-windows.ps1 -Mode test
~~~

For complete adversarial test coverage and safety prerequisites, see
[TESTING.md](TESTING.md). For the OpenAI/Anthropic tool protocol, see
[TOOL_INTEGRATIONS.md](TOOL_INTEGRATIONS.md). For website preview, hosting,
deployment, and site checks, see
[WEBSITE_DEVELOPMENT.md](WEBSITE_DEVELOPMENT.md).

## CircleCI native continuous integration

The [CircleCI configuration](../.circleci/config.yml) follows the same
parameterized-workflow approach as Agent-RT but runs ReaTerm's **native Rust
sandbox** before the Python, TypeScript and website tests. The GitHub Actions
test workflows remain disabled; this is a separate CI integration.

Connect `Pro-GenAI/ReaTerm` as a CircleCI project with GitHub and use the
repository's `.circleci/config.yml`. The configuration cannot start jobs
until the CircleCI project is connected and has access to the required
resource classes. It does not configure a project automatically.

| Workflow | Executor | Default | Verification |
| --- | --- | --- | --- |
| `linux_ci` | Ubuntu 24.04 x86-64 VM | On | Rust Landlock/seccomp, Python, TypeScript, website, scripts |
| `linux_arm_ci` | Ubuntu 24.04 native ARM64 VM (`arm.large`) | Off | Same tests on AArch64 kernel sandbox |
| `apple_silicon_ci` | macOS Apple Silicon M4 Pro (`m4pro.medium`), Xcode 27.1 | On | Rust Seatbelt, Python, TypeScript, website, scripts |
| `windows_ci` | Windows Server 2022 x86-64 | On | Native PowerShell fail-fast runner including Windows token tests |

CircleCI pipeline parameters: set `run_arm_ci` to `true` to enable Linux
ARM64; `run_mac_ci` and `run_windows_ci` default to `true`.
`run_stress` defaults to `false` and, when enabled, runs the 10,001-entry
listing-limit regression on Linux x86-64. The default Windows workflow requires usable Windows CircleCI
credits/resources and must be monitored for native support.

Unix runners install Rust stable with rustfmt/Clippy, Python 3.11+ and Node
22, run offline Make/release safety tests, then run `make verify` in the
repository's required Rust → Python → TypeScript → website order. They
additionally perform a native read/deny smoke test and store the native
executable as a CircleCI artifact. Dependencies are cached using Cargo
and npm lockfiles. Set `REATERM_DECISION_MODEL` to empty in the tests so
no real model provider is contacted. Windows uses
`scripts/verify-windows.ps1 -Mode verify` and also stores its executable.
There are no publishing or deployment steps.

**The Linux ARM64 runner is not a Raspberry Pi**: it verifies native
AArch64 execution and the CI kernel's Landlock/seccomp support, not a Pi's
4 GB limit or its particular OS configuration. Run the native tests on the
actual Pi to verify its enforcement. CircleCI's hosted macOS Intel
executors are retired; macOS Intel requires a separately provisioned
self-hosted CircleCI runner or manual verification. The static OS badges
in the READMEs are not real-time CI status indicators.

With the CircleCI CLI installed, validate changes using:

~~~bash
circleci config validate .circleci/config.yml
circleci run trigger --param run_arm_ci=true
~~~

## Documentation ownership

| Document | Audience | Purpose |
| --- | --- | --- |
| [Root README](../README.md) | Prospective users | Benefits, short examples, integrations |
| [Python README](../src/python/README.md) | Python users | Python value and simple API usage |
| [Node.js README](../src/typescript/README.md) | npm users | Node.js value and simple API usage |
| [Website README](website/README.md) | Site visitors | Website and product overview |
| **DEVELOPMENT.md** | Contributors | Setup, builds, dev workflow, releases |
| [TESTING.md](TESTING.md) | Contributors | Test matrix, sandbox assertions, stress tests |
| [WEBSITE_DEVELOPMENT.md](WEBSITE_DEVELOPMENT.md) | Site maintainers | Preview, hosting, deployment, site tests |
| [TOOL_INTEGRATIONS.md](TOOL_INTEGRATIONS.md) | API integrators | Provider tool schemas and execution |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Security reviewers | Threats, scope and limitations |
| [AGENTS.md](../AGENTS.md) | Coding agents | Repository invariants and workflow |

When a contributor procedure changes, update the appropriate developer
guide rather than adding maintenance instructions to a README.


## Supported platforms and building native binaries

ReaTerm targets **five 64-bit OS/CPU combinations**. The Rust command
allowlist, capability-scoped file paths and resource caps are the same on
all of them, but the kernel sandbox differs:

| Platform | Rust target triple | Native sandbox | CI runner |
|---|---|---|---|
| Linux x86-64 (Intel/AMD) | `x86_64-unknown-linux-gnu` | Landlock ABI v3+ and seccomp | `ubuntu-latest` |
| Linux ARM64 (e.g. Raspberry Pi with 64-bit OS) | `aarch64-unknown-linux-gnu` | Landlock ABI v3+ and seccomp | `ubuntu-24.04-arm` |
| macOS Intel | `x86_64-apple-darwin` | macOS Seatbelt | `macos-15-intel` |
| macOS Apple Silicon (M1/M2/M3/M4+) | `aarch64-apple-darwin` | macOS Seatbelt | `macos-15` |
| Windows Intel/AMD (x86-64) | `x86_64-pc-windows-msvc` | Windows write-restricted access token | `windows-2022` |

**32-bit Raspberry Pi OS/ARMv7 and other unlisted targets are unsupported.**
Each supported platform fails closed when its OS enforcement is unavailable. The Python and TypeScript clients
use the *same* platform-native Rust executable; they do not replace it.

On Linux and macOS, after installing Rust, build the native binary as follows;
for Windows use the PowerShell/MSVC commands in section 5.

    cargo build --release --manifest-path src/rust/Cargo.toml
    ./src/rust/target/release/reaterm --version
    ./src/rust/target/release/reaterm --root . ls -la

Or run `make setup` followed by `make test` at the repository root to build,
install wrapper dependencies and run all native integration tests. Use
`bash scripts/install.sh` to install the CLI into Cargo's bin directory.

### 1. Linux Intel/AMD (x86-64)

On an x86-64 Ubuntu/Debian machine with a Linux kernel providing Landlock
ABI v3+:

    sudo apt-get update
    sudo apt-get install -y build-essential curl make python3-venv
    # Install stable Rust with https://rustup.rs/ if cargo is not installed.
    rustup target add x86_64-unknown-linux-gnu
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target x86_64-unknown-linux-gnu
    ./src/rust/target/x86_64-unknown-linux-gnu/release/reaterm --root . ls

Use `uname -m` to confirm `x86_64`. Run `make setup` followed by `make test` for native tests.

### 2. macOS Intel (x86-64)

Install Xcode Command Line Tools and Rust from
[https://rustup.rs/](https://rustup.rs/) if they are not present:

    xcode-select --install
    rustup target add x86_64-apple-darwin
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target x86_64-apple-darwin
    ./src/rust/target/x86_64-apple-darwin/release/reaterm --root . ls

Check `uname -m` → `x86_64`. Install a compatible Node.js 20+ and Python
3.11+ to run the wrapper suites, then execute `make setup` and `make test`. The compiled
program uses a mandatory **Seatbelt** sandbox with rules denying
`file-write*`, hard linking, forking/exec and network calls. If macOS
refuses the sandbox profile, ReaTerm does not run the requested read command.

### 3. macOS Apple Silicon (arm64)

On Apple Silicon, `uname -m` normally prints `arm64`. From a native
Apple Silicon terminal (not a Rosetta-translated shell):

    xcode-select --install
    rustup target add aarch64-apple-darwin
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target aarch64-apple-darwin
    ./src/rust/target/aarch64-apple-darwin/release/reaterm --root . ls
    make setup
    make test

To create a **macOS universal binary** containing Intel and Apple Silicon
slices, build both targets on a macOS machine with the relevant Rust targets
and Xcode SDK, then combine them:

    rustup target add x86_64-apple-darwin aarch64-apple-darwin
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target x86_64-apple-darwin
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target aarch64-apple-darwin
    lipo -create \
      src/rust/target/x86_64-apple-darwin/release/reaterm \
      src/rust/target/aarch64-apple-darwin/release/reaterm \
      -output reaterm-macos-universal
    lipo -info reaterm-macos-universal

This is a local universal executable, **not** a signed/notarized macOS
distribution. Apple marks the `sandbox_init` API as deprecated; a future
OS version may remove or restrict it. Such a failure must remain fail-closed.
Do not remove Seatbelt or use an unrestricted fallback to make it work.

### 4. Linux ARM64 / Raspberry Pi (aarch64)

Use a Raspberry Pi that supports a **64-bit OS** (for example Pi 3/4/5 or
Zero 2 W with the appropriate 64-bit Raspberry Pi OS). Run:

    uname -m             # Must report aarch64, NOT armv7l
    uname -r             # Need Landlock ABI v3+ (normally Linux 6.2+)
    cat /sys/kernel/security/lsm    # Should include landlock

To build natively on the Pi:

    sudo apt-get update
    sudo apt-get install -y build-essential curl make python3-venv
    # Install stable Rust from https://rustup.rs/ if needed.
    rustup target add aarch64-unknown-linux-gnu
    cargo build --release --manifest-path src/rust/Cargo.toml \
      --target aarch64-unknown-linux-gnu
    ./src/rust/target/aarch64-unknown-linux-gnu/release/reaterm --root . ls

Run `make -C src/rust setup` and `make -C src/rust test` to test native
enforcement, or install Node 20+ and Python 3.11+ and run `make setup`
followed by `make test` for all languages. A Pi with
limited RAM may benefit from `CARGO_BUILD_JOBS=1` while compiling.
A 64-bit **kernel with a 32-bit userspace is not sufficient**: the Rust
program must run as AArch64. On kernels without Landlock, or where the
Landlock LSM is disabled, ReaTerm **refuses filesystem commands** rather
than running unsandboxed.

To **cross-compile** a Pi ARM64 binary from x86-64 Ubuntu/Debian:

    sudo apt-get install -y gcc-aarch64-linux-gnu libc6-dev-arm64-cross
    rustup target add aarch64-unknown-linux-gnu
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
      cargo build --release --manifest-path src/rust/Cargo.toml \
      --target aarch64-unknown-linux-gnu

Copy `src/rust/target/aarch64-unknown-linux-gnu/release/reaterm` to the
Pi (e.g. via scp), then test it **on the Pi**. Building a binary on another
host does not establish that the Pi's kernel supports Landlock/seccomp.


### 5. Windows Intel/AMD x86-64 (Windows 10/11)

Install stable Rust with the x86_64-pc-windows-msvc toolchain, Microsoft
Visual Studio C++ Build Tools plus a Windows SDK, and PowerShell (included
with Windows). Python 3.11+ and Node.js 20+ are needed for adapter tests.

From the repository checkout in PowerShell, build natively:

    rustup target add x86_64-pc-windows-msvc
    cargo build --release --manifest-path src/rust/Cargo.toml --target x86_64-pc-windows-msvc
    .\src\rust\target\x86_64-pc-windows-msvc\release\reaterm.exe --version
    .\src\rust\target\x86_64-pc-windows-msvc\release\reaterm.exe --root . ls -la

Alternatively install into Cargo's bin directory, typically on PATH:

    cargo install --path src/rust
    reaterm.exe --help
    reaterm.exe --root . cat README.md

Set up and test the native binary, adapters and website:

    .\scripts\verify-windows.ps1 -Mode setup
    .\scripts\verify-windows.ps1 -Mode test

Each command processes Rust, Python, TypeScript and the website in order,
stopping on failure. Add `-Language rust`, `-Language python`,
`-Language typescript` or `-Language website` to select one stage.

The Python and TypeScript verification steps require the release binary
from the native Rust setup. If GNU Make is installed (e.g. via MSYS2),
the root and per-language make setup and make test targets
delegate to the same PowerShell runner on Windows. The Bash installer
script is designed for Linux/macOS, not Windows.

Windows enforcement uses the native Rust allowlist plus a WRITE_RESTRICTED
access token created by Win32 CreateRestrictedToken, with an unpredictable
restricting SID and nonessential privileges removed. It is impersonated
on the current command thread before the filesystem root is opened, and
the runtime refuses to execute a read command if enforcement fails.
Regression tests attempt direct file changes and metadata mutations
against private disposable fixtures.

IMPORTANT: This Windows restriction is THREAD-scoped rather than a
process-wide sandbox. It does not control unrelated threads, child
processes, inherited writable handles, or filesystem objects with a
null DACL. The native dispatcher neither spawns processes nor creates
worker threads, but this is not a malware sandbox. Parent PowerShell
or cmd.exe can still write files through output redirection. See the
threat model for the exact guarantees and residual risks.

Windows on ARM64 and 32-bit Windows are not yet verified or supported.
The Windows-specific GitHub Actions workflow is currently disabled; run the PowerShell verification locally.
The original cross-platform CI workflow was deliberately disabled in
the repository; adding Windows support does not re-enable it.

### Verify OS sandbox support

For Linux, verify `uname -r`, the kernel's LSM list, and **actually run**
a read-only command. Container security policies can deny Landlock even
if the host kernel is recent. If `/sys/kernel/security/lsm` is missing,
inspect the kernel configuration and mounted `securityfs`; do not infer
the sandbox is active just from its absence or presence.

For macOS, verify the sandbox using the direct-syscall test that installs
Seatbelt in a dedicated child process:

    cargo test --manifest-path src/rust/Cargo.toml \
      seatbelt_blocks_direct_syscall_mutations_but_permits_reads -- --nocapture

All three operating systems reject write-looking commands in the Rust allowlist
before any external program can run. The **parent shell and inherited file
descriptors remain outside** the native process's protection; see
[THREAT_MODEL.md](THREAT_MODEL.md).

## Platform and prerequisites

The native CLI executes on Linux x86-64/ARM64 (Landlock ABI v3+ and seccomp),
macOS Intel/Apple Silicon (Seatbelt), and Windows x86-64 (a write-restricted
low-integrity thread token). Container restrictions or unavailable OS sandbox
services cause commands to fail closed. Windows ARM64 and 32-bit ARM
remain unsupported.

Requirements: Rust/Cargo 1.77+ (stable recommended), Python 3.11+, Node.js
20+ and npm. Do not patch around Landlock errors: a read-only command parser
alone does not provide the security boundary.

## Building and installing Rust

From the repository root:

    git clone https://github.com/Pro-GenAI/ReaTerm.git
    cd ReaTerm
    rustup toolchain install stable
    cargo build --release --manifest-path src/rust/Cargo.toml

The release executable is src/rust/target/release/reaterm on Linux/macOS,
or src/rust/target/release/reaterm.exe on Windows. Native artifacts may
be available from successful GitHub Actions runs. They are not committed
into the repository. Optionally install it on your PATH:

    cargo install --path src/rust
    reaterm --version

Examples:

    reaterm --root . pwd
    reaterm --root . ls -la
    reaterm --root . cat README.md
    reaterm --root . head -n 8 README.md
    reaterm --root . tail -n 8 README.md
    reaterm --root . wc -l README.md
    reaterm --root . grep -ni reaterm README.md
    reaterm --root . stat LICENSE

--root defaults to the current working directory. All operand paths must
be relative and cannot contain parent-traversal components or escape the
selected root.

## Supported commands and flags

- pwd: no arguments.
- ls: [-a] [-l] [PATH]; combined -la/-al allowed.
- cat: one or more regular files.
- head: [-n NUMBER] FILE; defaults to the first ten lines.
- tail: [-n NUMBER] FILE; defaults to the last ten lines.
- wc: [-l] FILE; count bytes/words/lines, or only lines.
- grep: [-n] [-i] TEXT FILE; **literal substring matching**, not regex.
- stat: PATH; prints file type and size.

Unknown commands and flags are denied, as are shell syntax, command
substitution, processes, arbitrary executables and write operations. Content
readers reject special files (devices, FIFOs, sockets) and final symlinks.
Per-file read limit: 10 MiB; output limit: 1 MiB; directory-listing limit:
10,000 entries; requested head/tail lines: at most 100,000.

## How enforcement works

1. The CLI receives argv; it never invokes a shell.
2. It opens the selected root directory as a cap-std Dir capability.
3. Linux applies Landlock ABI v3 and a seccomp syscall filter. macOS installs
   a Seatbelt no-write/no-process-exec/no-network profile. Installation
   failure stops execution; neither backend has an unrestricted fallback.
4. Native Rust functions read relative to the capability and apply size and
   output bounds. They do not launch external programs.

The root is selected by the caller; this is not a secrecy policy. Landlock
does not revoke rights on already-open file descriptors. Run from a trusted
launcher with appropriate inherited descriptors.

**Important:** the outer shell can still write files. For example:

    reaterm ls > file.txt

The calling shell may create/truncate file.txt **before ReaTerm starts**.
Likewise piping into tee can write. ReaTerm constrains only its own process,
not its launcher or other applications. See docs/THREAT_MODEL.md.

## Testing Rust

    cargo fmt --manifest-path src/rust/Cargo.toml --all -- --check
    cargo test --manifest-path src/rust/Cargo.toml
    cargo clippy --manifest-path src/rust/Cargo.toml --all-targets -- -D warnings
    cargo build --release --manifest-path src/rust/Cargo.toml

A disposable Linux or macOS smoke test:

    temp="$(mktemp -d)"
    printf 'hello\n' > "$temp/safe.txt"
    reaterm --root "$temp" cat safe.txt
    reaterm --root "$temp" ls
    reaterm --root "$temp" rm safe.txt      # must be denied
    reaterm --root "$temp" cat ../safe.txt  # must be denied
    cat "$temp/safe.txt"                    # still contains hello
    rm -rf "$temp"                          # parent cleanup, not ReaTerm

Unit tests verify policy primitives; run CLI smoke tests on the actual target
OS to test its kernel sandbox. The former Linux/macOS four-platform workflow
is disabled in this repository, as is the Windows-specific workflow.
Both remain available as test definitions for local or future CI use.

## One-line command API

The Python package exports `reaterm.run("ls -al")` (also `from reaterm import run`).
The npm ESM package exports `run("ls -al")` (also a namespace import:
`import * as reaterm`). Both conveniences tokenize quotes/whitespace
into argv and delegate to the existing ReaTerm class and native binary.
They do not run a shell: shell operators (`|`, `&&`, `>`), substitutions,
backticks and multiline input fail before invoking Rust.
Use the class API for exact literal arguments, including filenames with
otherwise rejected metacharacters. Python helper options: `root`,
`binary`, `timeout`, `decision_model`. Node helper options:
`ReaTermOptions` in the second parameter. Neither package bundles Rust.

## Python provider tool integration

The new `reaterm.tools.openai` module exports `reaterm_tool_openai` for OpenAI Chat Completions and `reaterm_tool_openai_responses` for Responses. `reaterm.tools.anthropic` exports `reaterm_tool_anthropic` for Anthropic Messages. The shared `execute_reaterm_tool` accepts OpenAI JSON-string arguments or an Anthropic mapping with exactly `{"command": "ls", "args": ["-al"]}` (or another native read-only command). It validates the shape and allowlisted name, then delegates to `ReaTerm.run(command, *args)` as literal argv without invoking a shell. The caller chooses `root`, `binary`, `timeout` and optional decision model; Rust remains the final authority for flags, filesystem paths, resource bounds and OS confinement. Provider schemas import without provider SDK dependencies. Returning tool output to an external model can disclose file contents.

See [TOOL_INTEGRATIONS.md](TOOL_INTEGRATIONS.md) for provider round trips. Tests are in `src/python/tests/test_tools.py` and include optional native checks when `REATERM_BIN` is configured.

## Python adapter

Install from the repository root:

    python -m pip install -e src/python
    export REATERM_BIN="$PWD/src/rust/target/release/reaterm"
    python -m unittest discover -s src/python/tests -v
    python -c 'from reaterm import ReaTerm; print(ReaTerm(root=".").run("ls", "-la"))'

ReaTerm.run(command, *args) returns stdout, raises ReaTermError on denied
operations, failed subprocesses or timeouts, and invokes subprocess.Popen with
an argv array and shell=False. Both output streams have a combined 2 MiB
capture cap and invalid UTF-8 is decoded with replacement characters.
Binary lookup order: explicit binary option,
REATERM_BIN, source-checkout target, then PATH. A deployment should pin a
trusted absolute binary path.

### Optional OpenAI-based veto — Python

    python -m pip install -e 'src/python[decision]'
    export REATERM_DECISION_MODEL='your-compatible-model-id'
    export REATERM_OPENAI_API_KEY='your-provider-key'
    export REATERM_OPENAI_BASE_URL='https://compatible.example/v1'

Uses the official OpenAI Python package chat completions API. Custom
REATERM_OPENAI_API_KEY / REATERM_OPENAI_BASE_URL override OPENAI_API_KEY /
OPENAI_BASE_URL fallbacks. A model must return exactly ALLOW; any other
response or error is a veto. The native Rust allowlist still determines
which operations can execute. The Rust binary does not call a model.

Enabling the model sends the command argument strings to the configured
provider. Assess confidentiality before using an external endpoint. No
model SDK is imported or network request made if REATERM_DECISION_MODEL
is unset.

## TypeScript/Node adapter

From the repository root:

    cd src/typescript
    npm install
    npm run build
    REATERM_BIN="$PWD/../rust/target/release/reaterm" npm test

In a consuming ESM application:

    import { ReaTerm } from "reaterm";
    const term = new ReaTerm({ root: "/path/to/project" });
    console.log(await term.run("cat", "README.md"));

The official openai npm package is an optional peer dependency, loaded
only when REATERM_DECISION_MODEL is configured. Set
REATERM_OPENAI_API_KEY/REATERM_OPENAI_BASE_URL (or OPENAI_API_KEY /
OPENAI_BASE_URL) and optionally constructor model/apiKey/baseURL.
Unexpected model output or API failure vetoes; it cannot grant operations.

The client uses child_process.spawn with an argv list, shell:false,
and enforces a bounded output capture. Never build a shell string from
untrusted arguments.

## Continuous integration and releases

The previous Linux/macOS workflow is retained as
.github/workflows/ci.yml.disabled, without automatic runs. Its most recent
fully successful four-platform run is available in the repository's GitHub
Actions history. The .github/workflows/windows.yml.disabled workflow
previously ran Windows x86-64 using the fail-fast PowerShell test pipeline,
including Rust, Python, TypeScript and website checks. Neither workflow
currently runs automatically; build the Windows executable locally. This is an ordinary CI artifact, not a signed release.
Formal supply-chain provenance, signed releases and locked dependencies are
future milestones.

## Troubleshooting

- Landlock unavailable: verify Linux >= 6.2, enabled LSM and container
  restrictions; **never** bypass the enforcement failure.
- Path denied: inspect --root, file permissions and symlink behavior.
- Unsupported command/flag: the allowlist is intentionally narrow.
- File or output limit: use a smaller target or more focused operation.
- Python/TypeScript ENOENT: compile Rust and set an absolute REATERM_BIN.
- Model veto unavailable: check official SDK, credentials, model name and
  endpoint API compatibility. Unset REATERM_DECISION_MODEL to use native
  deterministic policy without an additional model gate.


## Metadata mutation protection (required)

Landlock ABI v3 protects against file writes, creation, deletion and
truncation, but does **not** block permission or ownership changes on its
own. ReaTerm also installs a mandatory seccomp-BPF policy before running
native commands. The filter denies chmod/fchmod/fchmodat, chown/fchown,
extended-attribute mutation, timestamp updates, selected mount operations
and their alternate syscall forms. If this filter cannot be installed,
ReaTerm fails closed.

The Linux seccomp filter supports x86-64 and ARM64; other Linux architectures
fail closed. macOS uses a separate mandatory Seatbelt backend. No unrestricted
fallback or force-enable flag exists. To validate its behavior:

    cargo test --manifest-path src/rust/Cargo.toml
    cargo build --release --manifest-path src/rust/Cargo.toml

Rust integration tests launch the real executable, reject destructive
commands and their flags, and check that file contents, modes and directory
entries are unchanged. Additional unit tests attempt a chmod syscall after
installing the seccomp filter and confirm it receives EPERM.

Denied commands include rm, rmdir, cp, mv, touch, mkdir, chmod, chown,
chgrp, truncate, dd, tee, install, ln, sed, find, Git mutation subcommands,
bash, sh and Python. These examples are not an exhaustive blacklist:
ReaTerm only dispatches explicitly implemented read-only commands.

## Rigorous regression test suites

The tests are documented in [TESTING.md](TESTING.md), including an explicit
command/policy matrix, symlinks and special files, 1 MiB/10 MiB resource
boundaries, Linux seccomp metadata restrictions, optional model vetoes and
language bridge integration. All negative runtime cases operate only on
private disposable fixture files. They do not intentionally mutate host
files or use paid model APIs.

For an entire standard Linux regression run:

    bash scripts/test.sh

To include the slower 10,001-entry directory stress test:

    bash scripts/test.sh --stress

You may also run `cargo test --manifest-path src/rust/Cargo.toml --all`
for Rust alone. Set REATERM_BIN to the release executable when manually
running Python and TypeScript integration tests. The original Linux/macOS workflow is currently disabled; it previously
offered an opt-in stress test. The focused Windows workflow is currently disabled; use the PowerShell
runner for local Windows verification. For ARM64 or Raspberry Pi, run
these tests on an ARM64 host with the required kernel policy.

 
## CLI installation, global flags and exclusive informational requests

For a one-step Linux CLI installation, run from the repository root:

    bash scripts/install.sh
    reaterm --help
    reaterm ls -la
    reaterm cat README.md

This uses cargo install --path and places the binary in Cargo's bin
directory. If the shell cannot find reaterm, add the indicated directory
to PATH. To avoid installation, execute
./src/rust/target/release/reaterm after a release build.

Informational flags (each works only as the **sole** CLI argument):

| Flag | Action |
|---|---|
| -h, --help | Show comprehensive usage and commands |
| -V, --version | Show the compiled version |
| --list-commands | Print the exact read-only command allowlist |
| --limits | Print file, output, directory and line-count limits |
| --license | Print the license |
| --about | Print platform and sandbox requirements |

The parser scans **every argv token before opening the root or dispatching
a command**. Any informational flag alongside a command, another option,
a --root value, or another informational flag is an error (exit code 2,
diagnostic on stderr, no stdout or command execution). This also applies
when the information flag appears **after** a command or filename:

    reaterm --help                     # succeeds; usage only
    reaterm --version                  # succeeds; version only
    reaterm --help cat README.md       # error; does NOT read the file
    reaterm cat README.md --help       # error; does NOT read the file
    reaterm --version ls               # error; does NOT list files
    reaterm -h --version               # error; exclusive flags
    reaterm --root . --help            # error; information must be alone

The root selectors are *execution options*, not informational flags:
--root DIRECTORY, --root=DIRECTORY, and -C DIRECTORY. They must be
specified at most once and **before** the command. Default root is the
current working directory, so reaterm cat README.md works from a project
root without extra configuration. Unsupported global switches fail
closed, and command-specific flags remain subject to the native allowlist.

The dedicated Rust integration file tests/cli_flags.rs and the CLI parser
unit tests verify standalone modes, error statuses, output isolation,
order independence, root forms and absence of any file access before
an informational conflict is rejected.

## Optional model environment file

The repository provides a root-level `.env.example` with all supported
model settings. To configure an optional wrapper-only decision veto:

    cp .env.example .env
    # Fill in REATERM_DECISION_MODEL and REATERM_OPENAI_API_KEY
    # Optionally fill REATERM_OPENAI_BASE_URL for a compatible endpoint
    set -a
    . ./.env
    set +a

Both language wrappers read **exported process environment variables**;
neither automatically loads `.env`. The Rust compiled CLI does not perform
model decisions. Leave REATERM_DECISION_MODEL empty to disable model calls.
Never commit secrets: `.env` and local `.env.*` overrides are ignored by
Git; `.env.example` has placeholders only. Keep this file and AGENTS.md
in sync when configuration changes. The optional model receives command argv
and can veto requests but cannot bypass native read-only restrictions.

 
## Static project website

The site for **https://reaterm.pro-genai.com/** is maintained in
[website/](website/README.md). This is a static site with no build step.
Full website preview, deployment and configuration instructions live in
[WEBSITE_DEVELOPMENT.md](WEBSITE_DEVELOPMENT.md).
Preview it from the repository root:

    cd docs/website
    python -m http.server 8000

Open http://localhost:8000/. Publish the contents of docs/website/
as the static host's document root and configure the custom domain and
HTTPS separately. The source tree alone does not deploy the website.

The package metadata for Rust (Cargo.toml), Python (pyproject.toml), and
npm (package.json) carries the intended homepage URL. Their package
READMEs also link to the site. For source-only packages, do not claim
PyPI or npm publication until verified.

Website checks run as part of CI and can be executed alone:

    python -m unittest discover -s docs/website -p "test_*.py" -v

Keep AGENTS.md and docs/WEBSITE_DEVELOPMENT.md updated with changes to
hosting, packages, commands, OS requirements, or website architecture.
Keep docs/website/README.md as a user-facing product introduction.


## Make-based developer workflow

From the **repository root** on Linux:

    make setup    # Build Rust release binary, install Python in src/python/.venv,
                  # install npm dependencies, prepare static website
    make test     # Rust format/Clippy/tests, Python tests, npm build/tests, website

Each command processes Rust, Python, TypeScript and the website in order,
stopping on the first failure. Run the tests only after setup succeeds.

Per-language commands (run from the repository root):

    make -C src/rust setup
    make -C src/rust test

    make -C src/python setup
    make -C src/python test

    make -C src/typescript setup
    make -C src/typescript test

    make -C docs/website setup
    make -C docs/website test

`src/rust/Makefile` builds a release binary in `src/rust/target/release/`
rather than installing to global PATH. The Python setup creates
`src/python/.venv` and installs the package in editable mode there;
it does not modify system Python. Python 3.11 or newer is required. If
`python3` is older (as on some macOS installations), select a supported
interpreter with `make verify PYTHON=python3.11`. The setup upgrades pip in
the virtualenv before installing the package. If an existing virtualenv uses
an older Python, remove `src/python/.venv` and rerun setup. The TypeScript setup uses
`npm install --no-audit --no-fund`. The website is dependency-free.

A language-specific Python/TypeScript `make test` expects
the **compiled Rust release binary** already to exist, because integration
tests must exercise the native enforcement. Run Rust setup first, or use
root `make setup`. `REATERM_BIN` may be overridden to a trusted absolute
binary location. For Python, `PYTHON` and `VENV_DIR` are configurable;
for TypeScript, `NPM` is configurable; for Rust, `CARGO` is configurable.

The Make targets are **development build/install commands**; they write
dependencies and build artifacts. Only the ReaTerm runtime is read-only.
The test targets clear `REATERM_DECISION_MODEL` so they never accidentally
make an external AI model request.

`scripts/test_make_workflow.py` independently verifies all ordering and
fail-fast properties using temporary fake language Makefiles, including
parallel Make invocation. Run it without installing toolchains:

    python3 -m unittest discover -s scripts -p 'test_make*.py' -v

The Make-based end-to-end verification and its orchestration tests also run
in GitHub Actions.


## Direct-to-main workflow

The repository owner requires commits pushed **directly to `main`, without pull requests**. Fetch `main` before editing, run applicable tests, and push a fast-forward commit only. Do not force-push, discard concurrent changes, or bypass branch rules; report failures to update `main`. PyPI/npm publication remains a separate explicit operation.

## Publishing ReaTerm packages

**Publication is separate from development tests** and is irreversible for
an already-uploaded version. Publishing does not bundle the Rust native
executable into either wrapper package. Consumers must install a compatible
ReaTerm CLI separately and set REATERM_BIN or install it on PATH.

### Shared release version and preflight

Keep the versions aligned in:

- `src/python/pyproject.toml` — PyPI distribution name `reaterm`
- `src/rust/Cargo.toml` — Rust crate and executable name `reaterm`
- `src/typescript/package.json` — unscoped npm package name `reaterm`

Run `scripts/update-versions.sh` to increment the patch version, or pass an explicit
`major.minor.patch` version. It updates the three manifests, the Rust
Cargo.lock, and the npm lockfile only if one is present; it does not
publish anything. The default checkout does not commit package-lock.json.

Use a GitHub release tag matching the version, e.g. `v0.1.0`. A mismatch
between any version and the release tag prevents PyPI publication. Run
`make setup` and `make test` on supported Linux/macOS systems, or use
`scripts/verify-windows.ps1 -Mode setup` and `-Mode test` on Windows.

Validate the publishing configuration and mocked npm behavior offline:

    python3 -m unittest discover -s scripts -p 'test_publish*.py' -v

The test suite exercises the npm dry-run, default publish, failure
propagation, package manifest validation and temporary credential cleanup
with a fake npm executable. **It never uploads to a registry.**

### PyPI: release event with trusted OIDC publishing

`.github/workflows/publish-pypi.yml` publishes **only** when a GitHub
release is marked **published**. It is independent of the intentionally
disabled `ci.yml.disabled` and `windows.yml.disabled` test workflows.

Before creating a release, configure **PyPI Trusted Publishing** for
project `reaterm` with repository owner `Pro-GenAI`, repository
`ReaTerm`, workflow filename `publish-pypi.yml`, and GitHub
environment `pypi`. On the GitHub side create an environment named
`pypi`; environment approval rules are recommended. A first upload
may require a PyPI pending-publisher/project setup. No PyPI API token
needs to be stored in GitHub secrets.

The workflow checks out the release tag, checks Python/Rust/npm names and
versions, runs the native Linux sandbox test suite, builds a Rust release
binary and the Python **wheel and sdist** from `src/python`, validates
the archives using Twine, installs the built wheel into an isolated
venv, and runs all Python tests with `REATERM_BIN` pointing to the real
Rust executable. Only if this succeeds is the verified distribution
artifact sent to the separate OIDC publish job.

**Do not publish a GitHub release unless you intend to publish the matching
PyPI version.** Python package installation alone does not install the
Rust binary. PyPI will not allow reusing an already-published version.

### npm: publish from the TypeScript directory

The repository does not commit `src/typescript/package-lock.json` or a
pnpm lockfile, so the publishing script uses `npm install
--no-package-lock --no-audit --no-fund`, not `npm ci`/pnpm. Unlike
a lockfile-backed release, dependency resolution is not fully pinned.
The script builds the native Rust CLI when REATERM_BIN is unset, and
executes `npm run check` with the native executable present so security
integration tests cannot silently skip. It inspects the `npm pack`
manifest, requiring compiled JavaScript, type declarations, README and
package metadata, and rejecting common secret files.

From the **repository root**:

    # Validate WITHOUT logging in or publishing
    bash src/typescript/scripts/npm_publish.sh --dry-run

    # Real publication by default: run ONLY when ready
    NPM_TOKEN=... bash src/typescript/scripts/npm_publish.sh
    bash src/typescript/scripts/npm_publish.sh --publish --tag next

Publishing to the unscoped npm name `reaterm` requires control of that
name on the target registry; first verify it is available or owned by the
publishing account. Unscoped npm packages are public. Authenticate using an appropriate
`NPM_TOKEN` or `npm login`. Set `NPM_REGISTRY` only if you intentionally
target another **HTTPS** registry. The script creates any token-bearing
configuration in a restricted temporary folder **outside the repository**,
removes it on exit and does not print the token. No authentication is
attempted in dry-run mode.

The script publishes by default; `--dry-run` prevents uploading.
Published package name, version and repository homepage come
from the checked-in `src/typescript/package.json`.


## Security hardening and distribution status

Native Linux Landlock read permission is restricted to the operator-selected
`root_path`; Linux seccomp additionally denies process spawning, socket
creation/connection, io_uring setup, ftruncate, metadata changes and mounts.
The native read API streams `head`, `tail` and `grep` and keeps the 10 MiB
file and 1 MiB output caps; `grep` now accepts up to 128 explicit files.
Ordinary `make verify` includes the added seccomp, filename, lexer and
SDK regression tests, but not opt-in stress/fault-injection tests.

The Windows native path remains capability-scoped through cap-std. Unlike
Unix, cap-std's OpenOptionsExt does not expose a Windows
`FILE_FLAG_OPEN_REPARSE_POINT` control. Do not substitute an absolute-path
`std::fs::OpenOptions` open, which would compromise confinement.
Windows content opens continue to reject inspected final symlinks and check
the opened file type; an atomic Windows no-follow enhancement requires
a capability-relative implementation and native Windows race tests.

Neither the Python distribution nor the npm distribution currently bundles
platform-native Rust executables. Platform-specific wheels and optional npm
binary packages require reproducible native builds, signing, wheel/npm
installation tests for each supported platform and a separately authorized
publishing action. Do not claim toolchain-free installation until those
artifacts have been produced and tested.
