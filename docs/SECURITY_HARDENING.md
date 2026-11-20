# ReaTerm security, operational limits and alternatives

ReaTerm is an **unaudited prototype**, suitable for controlled experiments
with a small operator-selected project root. It is not an isolation boundary
for running arbitrary hostile code, and read-only execution does not make
file content safe to send to an AI model.

## Choosing the right tool

| Tool | What it can provide | Principal trade-off |
| --- | --- | --- |
| ReaTerm | Small fixed native inspection command set, scoped root, OS write enforcement, capped output; Python/TypeScript/MCP adapters | Requires a separately installed binary and working platform sandbox; read-only is not confidentiality |
| Read-only container bind mount | Filesystem mount-level write protection with familiar deployment tooling | Requires a container runtime and correct mounts/identity/network configuration; does not by itself block disclosure or prompt injection |
| bubblewrap / namespaces (Linux) | Flexible filesystem/process/network isolation for selected commands | Linux-focused, environment-specific setup and policies; more flexible commands increase policy scope |
| Framework-native read/grep tools | Simple structured input and a small code surface; no shell parser needed | Safety depends on the framework's path checks and host OS permissions; many tools do not install independent OS write protection |

**Use ReaTerm** when a local agent requires a standardized, auditable set of
inspection operations across several host OSes and you can install the native
binary and verify sandbox availability. Prefer a container/read-only mount
or a framework's simpler tools when they already meet the same requirements;
the combinations can be complementary. Do not infer stronger confidentiality
or code-execution isolation than the tool implements.

## What the default sensitive-path policy does

- Denies common credential directories and files across all read and metadata
  commands; also hides those names from directory listings, even with `ls -a`.
- Blocks selected roots that themselves contain a sensitive path component.
- Rejects static symlink traversal for reads and ancestor paths in listing
  or metadata operations.
- Allows conventional examples such as `.env.example`; such a file can still
  contain real secrets. Filenames are heuristics and can be false positives.

**Not guaranteed:** classification or redaction of secrets in ordinary files,
protection against renamed files or concurrent filesystem mutation, model
prompt-injection resistance, network exfiltration prevention by other tools,
or access control beyond the operating-system identity. There is currently
no built-in operator allowlist override. Select a separate minimized export
directory if the default deny policy blocks a legitimate file.

## Command interfaces

The public Python and Node convenience calls, for example
`run('cat "README.md"')`, tokenize a restricted command string. They do
not invoke a shell and reject shell operators. For exact inputs, use
`ReaTerm.run("cat", "README.md")`; the OpenAI/Anthropic tool and MCP
definitions already expose a structured `command` enumeration and
`args` string array. Model calls cannot supply `--root`, choose a native
binary or extend the allowlist.

## Resource and platform requirements

| Limit | Enforcement |
| --- | --- |
| Per-file content | 10 MiB |
| Native stdout | 1 MiB |
| Directory listing | 10,000 entries |
| Head/tail request | 100,000 lines |
| Native Unix CPU | 10 seconds soft / 11 seconds hard using `RLIMIT_CPU` |
| Python and Node native child process | 15-second wall timeout by default; configurable by a trusted caller |
| Native Windows CPU/wall time | **No native hard limit**; run through a supervising client |
| Slow/blocked filesystem reads | CPU cap alone does not terminate blocking I/O; host supervision required |

A Linux native sandbox requires Landlock ABI **v3** (introduced around
Linux 6.2) and seccomp on a supported architecture. The executable
**fails closed** on an unsupported kernel or policy-restricted container;
there is no unsafe compatibility fallback. macOS depends on deprecated
Seatbelt `sandbox_init`. Windows uses thread impersonation, not a
process-wide sandbox.

The locked dependencies use `cap-std` and `cap-primitives` version 3.4.6,
which fixes [RUSTSEC-2026-0482](https://rustsec.org/advisories/RUSTSEC-2026-0482.html).
Keep the lockfile for native releases and run vulnerability audits when
updating. Support for older cap-std 3.x releases is explicitly excluded.

## Deployment checklist

1. Export only necessary non-sensitive project files to a separate root.
2. Keep the native executable and its configuration operator-controlled.
3. Run `cargo test --locked --manifest-path src/rust/Cargo.toml --all`,
   Python unit tests, Node tests and native OS security tests in CI.
4. Verify Landlock/Seatbelt/restricted-token setup fails closed on each
   intended target OS.
5. Isolate or disable other agent tools capable of outbound communication;
   treat ReaTerm results as data and independently authorize side effects.
6. Supervise native child processes with wall timeouts; impose OS/container
   memory and I/O limits when handling untrusted filesystems.
7. Obtain an independent review and adversarial testing before making a
   production security claim.

See [Threat model](THREAT_MODEL.md) and [Testing](TESTING.md).
