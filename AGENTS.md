# ReaTerm Agent and Contributor Instructions

## Git workflow (repository owner directive)

**Always commit and push directly to `main`; do not create pull requests for repository changes.** Read the current `main` head before changing files, preserve other contributors' commits, run relevant checks, and make only normal fast-forward updates. Never force-push, reset `main`, or bypass branch protections. If a concurrent change or protection rule blocks the push, report the blocker instead of opening a PR or discarding changes. Publishing to PyPI/npm requires a separate explicit release action; ordinary pushes to `main` must not publish.

## README content policy (all README files)

**Every README must attract and orient users, not teach project
development.** This includes the root README, Python and npm package
READMEs, and docs/website/README.md. Use concise product benefits,
clear user-facing usage examples, realistic security boundaries and
links to integrations. Avoid build/test instructions, contributor
checklists, CI detail, release/publishing mechanics, local website
preview or deployment procedures in **any** README. Move that material
to docs/DEVELOPMENT.md, docs/TESTING.md, or
docs/WEBSITE_DEVELOPMENT.md. Developer documentation is the single
source of truth for maintenance, testing, and shipping.

## Core invariants

ReaTerm is a read-only native command interface, NOT a shell wrapper.
Every modification must preserve:

1. **No writes by native commands:** do not add filesystem mutation APIs,
   subprocess creation, shell interpreters, hooks, plugins or network clients
   to src/rust without a separate security design and review.
2. **Default-deny:** explicitly support each command and argument/flag. Do not
   classify command safety by checking for a handful of forbidden strings.
   **Informational CLI flags are exclusive:** --help/-h, --version/-V,
   --list-commands, --limits, --license, --about must never execute a
   command if combined with any other argument. Validate the ENTIRE argv
   before root resolution, filesystem access or command dispatch; reject
   conflicts on stderr with a nonzero exit status, no stdout.
3. **Kernel enforcement:** Linux x86-64 and ARM64 require root-scoped Landlock ABI v3+
   AND seccomp denial of metadata mutations (chmod/chown/xattr/timestamps),
   process creation (exec/fork/clone), socket/connect, io_uring setup and
   ftruncate.
   macOS Intel and Apple Silicon require a Seatbelt profile denying
   file-write*, file-link, process-exec/fork and network operations.
   Apple's sandbox_init API is deprecated; fail closed if unavailable.
   Windows x86-64 uses a low-integrity WRITE_RESTRICTED thread token
   with a fresh random restricting SID and dropped privileges, installed
   BEFORE opening the capability root. It is not process-wide: do not
   add worker threads, child processes, or inherited writable handles.
   Never downgrade to an unconfined fallback.
4. **Capability-relative paths:** always resolve files with cap-std Dir.
   Reject absolute and parent-traversal paths. Do not replace capability
   resolution with canonicalization+prefix checks vulnerable to races.
5. **Special files:** deny reading devices, FIFOs, sockets and final symlinks
   through content-reading commands. Reject intermediate symlink components
   for file and metadata/listing operations, without claiming those checks
   eliminate races in a concurrently modified filesystem. Deny common secret
   names and directories in native Rust operand/root validation and hide
   them from listings, including `ls -a`. This is filename-based,
   not content scanning/redaction/DLP, and can have false positives.
6. **Resource bounds:** enforce file size, output, line count and listing caps.
   Count every directory entry scanned, including filtered names. On Linux
   and macOS install the mandatory native 10/11-second soft/hard RLIMIT_CPU;
   wrappers default to 15-second subprocess timeouts. Unix CPU limits do
   not prevent blocking filesystem I/O; no native Windows deadline exists.
   The trusted host must supply additional wall-clock, memory and I/O limits.
7. **Model veto only:** optional OpenAI-compatible checks in the language
   wrappers can reject requests but never override Rust denial. Model errors
   fail closed. Model-facing tools can propose only native read-only commands;
   the caller must dispatch them and the model must never select the root.
8. **Argv only:** adapters must use subprocess.run(shell=False) or spawn
   (shell:false); never interpolate tokens into a shell string. One-string
   package-level helpers may only lex argv (quotes, whitespace, escapes),
   reject shell operators/substitutions, and delegate to existing class run.

## Repository layout

- Makefile: root setup/test/verify targets; verify MUST remain a sequential,
  interleaved Rust -> Python -> TypeScript -> website setup/test chain.
- src/rust/Makefile, src/python/Makefile, src/typescript/Makefile,
  docs/website/Makefile: local setup, test, verify entry points.
- scripts/test_make_workflow.py: offline fake-install and test-failure
  orchestration regressions, including parallel make -j.
- src/rust/: canonical crate and compiled executable named reaterm.
- src/rust/src/macos_guard.rs: macOS Seatbelt enforcement and child-process
  direct-syscall mutation tests.
- src/rust/src/metadata_guard.rs: Linux x86-64 and ARM64 seccomp filters.
- src/rust/src/windows_guard.rs: Windows write-restricted access-token
  enforcement and dedicated disposable file-mutation tests.
- src/rust/tests/windows_readonly.rs: Windows integration safety matrix.
- src/rust/tests/security_matrix.rs: cross-platform native integration
  regressions for traversal/absolute paths in every file reader, denied
  shells/writes, and informational-flag validation before root access.
  Disposable fixtures must retain contents, metadata and entry counts.
- scripts/verify-windows.ps1: native Windows setup/test/verify runner.
- .github/workflows/windows.yml.disabled: Windows x86-64 verification
  workflow, currently disabled. Do not re-enable without approval.
- docs/DEVELOPMENT.md: supported platform matrix and native/cross builds.
- src/python/: Python 3.11+ interface with optional official openai dependency.
  Package-level `reaterm.run("ls -al", root=".")` shlex-tokenizes a single
  string into argv; class `ReaTerm.run("ls", "-al")` keeps literal-argv behavior.
- src/python/reaterm/mcp_server.py: stdio MCP server using optional official
  MCP Python SDK v2 (mcp>=2,<3). It exposes only `run_command`, delegates
  to the shared strict tool executor, requires an operator-selected
  `--root`, and optionally accepts a trusted `--binary`. The MCP caller
  cannot select root/binary, spawn a shell, or run arbitrary executables.
  The console script is `reaterm-mcp`; test using
  src/python/tests/test_mcp_server.py with an offline fake SDK.
- src/python/reaterm/tools/: Python OpenAI Chat Completions / Responses
  and Anthropic Messages tool schemas, plus shared `execute_reaterm_tool`.
  Tool schemas have no required provider SDK. Inputs include exactly a
  native read-only `command` and an `args` string list; the caller owns
  `root`, `binary`, and model-veto settings. The MCP adapter uses the same
  payload and only stdio transport; its SDK dependency is optional.
  Tool output must be submitted
  explicitly to the provider. Test in src/python/tests/test_tools.py.
- src/typescript/: Node.js ESM/TypeScript interface; optional official openai.
  Exported `run("ls -al", options?)` uses quote/whitespace argv parsing;
  class `ReaTerm.run("ls", "-al")` keeps literal-argv behavior.
  `src/tools.ts` exports OpenAI Chat/Responses and Anthropic strict tool
  schemas plus a validating executor; `src/mcp-server.ts` exposes one
  stdio MCP `run_command` via optional official v2 SDK and Zod v4.
  The MCP handler MUST inherit the operator's REATERM_DECISION_MODEL;
  never force decisionModel to empty or bypass model veto.
  MCP protocol tests MUST drive real SDK clients through real transports:
  Python `pytest-mcptest==0.1.1` stdio fixtures, TypeScript
  `@modelcontextprotocol/client` in-memory and stdio. Keep separate lightweight
  handler mocks if useful, but do not substitute them for protocol testing.
  `make verify` and Windows verification run both Python unittest and
  pytest MCP suites; TS `npm run check` exercises the official MCP client.
  The host selects --root/--binary, never model input. The TS package
  publishes the optional `reaterm-mcp` bin and still requires the separate
  native executable.
- src/python/tests/test_security_matrix.py and
  src/typescript/test/security-matrix.test.ts: cross-language regression
  matrices for trusted-root confinement, command allowlists, literal argv,
  denied native operations and unchanged fixtures. Python offline mocks
  and TypeScript runtime validation run without a native binary. Native
  integration cases require REATERM_BIN; no external model calls.
- README.md, src/python/README.md, src/typescript/README.md and
  docs/website/README.md: public-facing marketing/usage only; never include
  developer setup, test commands, release or deployment instructions.
  Keep short examples truthful, mention the separate native executable,
  and avoid unverified claims of package registry availability.
  Keep the GitHub-backed MIT license badge. Do not display live PyPI/npm
  version badges before registry publication is verified. Avoid static
  platform-tested badges until their claims can be verified with current
  evidence. Explicitly distinguish source examples from published package
  availability. Link to docs/DEVELOPMENT.md for installation and testing.
- docs/DEVELOPMENT.md: setup, build, test and troubleshooting.
- docs/TOOL_INTEGRATIONS.md: provider schemas, tool-call/result cycles, and confidentiality risks.
- docs/website/: self-contained, deployable static website (index.html,
  styles.css, app.js, assets, 404, sitemap) for reaterm.pro-genai.com.
- docs/website/README.md: concise website and product introduction only.
- docs/WEBSITE_DEVELOPMENT.md: local preview, static hosting, domain/TLS,
  deployment, website test commands and maintenance instructions.
- docs/website/test_site.py: dependency-free site tests and HTTP smoke checks.
- docs/TESTING.md: rigorous security-test matrix, prerequisites and opt-in stress runs.
- scripts/test.sh: single-command full Linux regression runner.
- scripts/install.sh: optional cargo install helper for invoking reaterm from PATH.
- src/rust/src/cli.rs: validated CLI parser and exclusive information-only modes.
- assets/reaterm-cover.svg: self-contained horizontal README hero;
  update it when the architecture, supported platforms, or branding changes.
- .env.example: safe, up-to-date template for optional decision-model settings;
  never place real API keys in this file.
- docs/THREAT_MODEL.md: security boundaries and non-goals.
- docs/SECURITY_HARDENING.md: comparison with containers, bubblewrap and
  framework-native file tools, plus remaining confidentiality/CPU risks.
- src/rust/tests/secret_paths.rs: native secret-path deny regressions on
  supported platforms; tests/paths_and_policy.rs also covers FIFO failures.
- src/rust/src/lib.rs: native path validation, best-effort secret-name
  filter (including credentials, kubeconfig and keystore extensions),
  symlink-component rejection, `--` separators, bounded streaming
  head/tail/grep (up to 128 grep files), read/listing and Unix CPU cap.
  Linux Landlock must grant read access ONLY inside root_path, not `/`.
  Windows cap-std lacks a public Windows nofollow open flag; preserve
  capability-relative opens and document the remaining reparse race risk.
- src/rust/Cargo.toml: require patched cap-std 3.4.6+ on the 3.x line;
  retain the Cargo.lock pin because of RUSTSEC-2026-0482.
- .circleci/config.yml: native CircleCI regression pipelines, based on the
  Agent-RT configuration pattern. Linux x86-64 runs by default; native Linux
  ARM64 and macOS Apple Silicon (M4 Pro) are default-on parameterized
  workflows. Windows Server 2022 x86-64 defaults on with run_windows_ci=true;
  slow resource stress is opt-in with run_stress=true. Every enabled job
  runs the actual Rust CLI sandbox and full language/website regression
  suite; the Unix jobs use sequential make verify and cache registry data.
  Do not run native sandbox tests in Docker or claim native Raspberry Pi
  kernel verification from an ARM64 VM. CircleCI-managed Intel macOS
  executors are retired; Intel verification needs a separately configured
  self-hosted runner. CI never publishes packages or deploys a website.
- .github/workflows/ci.yml.disabled: intentionally disabled existing
  cross-platform GitHub Actions CI; do not silently re-enable it.
- .github/workflows/publish-pypi.yml: active GitHub-release-triggered PyPI
  Trusted Publishing workflow for the `reaterm` wheel/sdist. Requires
  matching `vVERSION` tag and versions in Python/Rust/npm; tests the
  native Linux CLI and clean Python wheel before granting OIDC.
- src/typescript/scripts/npm_publish.sh: npm-only unscoped
  `reaterm` publishing helper. It publishes by default;
  use `--dry-run` for validation without uploading. Validate built JS/types,
  native Rust integration, package contents and auth file cleanup.
- scripts/test_publish_workflows.py: offline fake-npm release-safety tests.
- scripts/update-versions.sh: update shared Python, Rust and npm release versions without publishing. The npm package-lock.json is optional; never
  assume it exists or require npm ci when absent.
- src/shared/tokenizer-fixtures.json: shared Python `shlex` and TypeScript
  tokenizer regression corpus for valid quote/escape parsing and denied
  metacharacters; standard language suites must exercise it.
- src/typescript/test/mcp-stdio.test.ts: actual MCP JSON-RPC stdio
  regression; a configured veto without model credentials MUST reject
  before any native command is launched.
- src/python/reaterm/client.py: guard shallow install ancestors, limit
  combined stdout/stderr capture to 2 MiB with replacement UTF-8 decoding,
  preserve argv-only process launch and trusted binary selection.
  Higher-level offline Python tool/MCP tests mock _capture_capped, not
  subprocess.run; inspect shell=False at the subprocess.Popen boundary.

## Contributor workflow

1. Read README.md and docs/THREAT_MODEL.md before security changes.
   Apply the direct-to-main workflow above; do not create PRs.
2. Implement policy/operations in Rust first; keep wrappers very thin.
3. Add tests for both allowed operations and denied writes, new flags, path
   escapes, symlinks and huge outputs. Test against disposable fixture trees.
4. Run `make verify` from the root for the full standard suite (setup and
   tests for each language, then website). A setup failure must prevent that
   language's tests; a test failure must prevent any later language.
   Never refactor the root verify recipe into parallel prerequisites.
   Run `python3 -m unittest discover -s scripts -p 'test_make*.py' -v`
   to test fail-fast order without toolchain installation.
   The `bash scripts/test.sh` compatibility runner also works. For a slower
   optional test of the listing limit, run `bash scripts/test.sh --stress`.
   For targeted changes, run:
       cargo fmt --manifest-path src/rust/Cargo.toml --all -- --check
       cargo test --manifest-path src/rust/Cargo.toml
       cargo clippy --manifest-path src/rust/Cargo.toml --all-targets -- -D warnings
       python -m unittest discover -s src/python/tests -v
       cd src/typescript && npm install && npm run check
5. Run an executable smoke test on a Linux host with Landlock and seccomp
   enabled. Native integration tests fail closed when enforcement is absent.
6. Update docs/TESTING.md and docs/DEVELOPMENT.md for new commands, policy
   rules, environment variables and test categories. Keep tests disposable
   and avoid external model requests; simulated model calls use local mocks.
7. Test standalone and mixed CLI flags with Rust tests/cli_flags.rs;
   verify both flag-before-command and flag-after-command forms are
   rejected without dispatch. Preserve --root DIR, --root=DIR and -C DIR
   as execution-only options that precede the command.
8. Keep every README focused on persuading potential users through benefits,
   minimal working usage snippets and accurate security boundaries. Do not
   add developer/test/setup/CI/publishing/deployment content to any README.
   Move it to docs/DEVELOPMENT.md, docs/TESTING.md or
   docs/WEBSITE_DEVELOPMENT.md. Maintain the hero asset and marketing examples
   as APIs change; do not claim the parent shell is sandboxed.
9. Keep website and package links current: source package READMEs and
   Python/npm/Rust package metadata should reference the website homepage.
   Do not claim PyPI/npm publication without verifying it. Keep docs/website
   dependency-free and accurate about kernel/security boundaries. Run
   python -m unittest discover -s docs/website -p "test_*.py" -v
   when changing website files. Never commit secrets or analytics code.
   On Windows, run scripts/verify-windows.ps1 -Mode verify to test the
   Rust, Python, TypeScript and website stages in fail-fast order.
   Run sandbox and integration tests on Linux x86-64/ARM64 and macOS Intel/
   Apple Silicon after platform-sensitive changes.
10. Publishing is high impact: maintain consistent package names and
    versions, never publish during tests, use PyPI OIDC trusted publisher
    and scope `pypi` environment, and keep npm auth outside the
    source tree. The Python package does not provide the Rust binary,
    nor does the npm package. Native prebuilt wheels/npm platform packages
    require a separately reviewed build matrix and explicit release. Run
    `python3 -m unittest discover -s scripts -p 'test_publish*.py' -v`
    for every release-tool change. Existing test CI workflows are
    intentionally disabled; do not reactivate them.
11. **Keep AGENTS.md current with every relevant change.** Whenever commands,
   architecture, security rules, dependencies, workflows, interfaces, or
   development instructions change, update this file in the same commit.
   Record the current conventions and key decisions so future contributors
   and coding agents can continue development without rediscovering context.

## Command policy

Native v0.1 operations: pwd, ls, cat, head, tail, wc, grep and stat.
grep is literal substring search, not a general regular expression engine.
All operand paths are relative to --root, selected by the invoker.

Adding git, find, sed, awk, python, node, sh, bash, tee, curl, xargs, shell
pipelines, process spawning, user-configured plugins or a general-purpose
interpreter requires separate architecture review. Many read-looking tools
can mutate files or invoke programs.

Never claim outer shell redirection is blocked. With
reaterm ls > file.txt, the parent shell can truncate file.txt before the
binary launches. ReaTerm constrains itself, not arbitrary other processes.

## Optional decision models

Keep root `.env.example` synchronized with Python/TypeScript environment
variables whenever model configuration changes. Actual `.env` secrets must
remain Git-ignored; neither adapter automatically loads `.env`.

REATERM_DECISION_MODEL activates *wrapper-only* model vetos. Use the
official OpenAI Python package and official openai npm package, respectively.
REATERM_OPENAI_API_KEY/REATERM_OPENAI_BASE_URL override the conventional
OPENAI_API_KEY/OPENAI_BASE_URL variables. A custom OpenAI-compatible endpoint
may be supplied. Without a model variable, no SDK import or network request
is made. Model responses must be exactly ALLOW to avoid a veto; the Rust
policy always applies independently.

## Security reporting

Report vulnerabilities privately. Do not post credentials, secrets or
private command content in public GitHub issues.
