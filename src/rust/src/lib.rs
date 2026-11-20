//! ReaTerm: a deliberately limited, native read-only command interface.
//!
//! No shell, subprocess, or file mutation API is used here. All paths are
//! resolved relative to a capability directory. Linux uses Landlock + seccomp;
//! macOS uses Seatbelt; Windows installs a mandatory write-restricted token.
//! The invoking *parent shell* is not subject to ReaTerm's restrictions.

mod cli;
#[cfg(target_os = "macos")]
mod macos_guard;
#[cfg(target_os = "linux")]
mod metadata_guard;
#[cfg(target_os = "windows")]
mod windows_guard;

use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use cap_std::fs::{OpenOptions, OpenOptionsExt};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_LINE_COUNT: usize = 100_000;

/// Best-effort default confidentiality guard, not a DLP or isolation boundary.
/// It applies to path operands and to names printed by directory listings.
fn sensitive_name(value: &str) -> bool {
    let name = value.to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".ssh"
            | ".aws"
            | ".gnupg"
            | ".kube"
            | ".azure"
            | "gcloud"
            | ".git"
            | ".docker"
            | ".npmrc"
            | ".pypirc"
            | ".netrc"
            | ".git-credentials"
            | ".pgpass"
            | ".my.cnf"
            | "kubeconfig"
            | "id_rsa"
            | "id_dsa"
            | "id_ecdsa"
            | "id_ed25519"
            | "credentials"
            | "credentials.json"
            | "application_default_credentials.json"
            | "service-account.json"
            | "service_account.json"
            | "secret"
            | "secrets"
            | "secrets.json"
            | "token"
            | "token.json"
    ) || (name.starts_with(".env.")
        && !matches!(name.as_str(), ".env.example" | ".env.sample" | ".env.template"))
        || name == ".env"
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".p12")
        || name.ends_with(".pfx")
        || name.ends_with(".jks")
        || name.ends_with(".keystore")
        || name.ends_with(".kdbx")
        || name.ends_with(".ppk")
        || name.ends_with(".tfstate")
        || name.contains(".tfstate.")
        || name.ends_with(".tfvars")
        || name.contains(".tfvars.")
}

fn sensitive_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => sensitive_name(&name.to_string_lossy()),
        _ => false,
    })
}

/// A counter that refuses excess output instead of writing unlimited data.
struct Output<W: Write> {
    inner: W,
    total: usize,
}
impl<W: Write> Output<W> {
    fn new(inner: W) -> Self {
        Self { inner, total: 0 }
    }
}
impl<W: Write> Write for Output<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.len() > MAX_OUTPUT_BYTES.saturating_sub(self.total) {
            return Err(std::io::Error::other("output exceeds 1 MiB limit"));
        }
        let n = self.inner.write(buf)?;
        self.total += n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn emit<W: Write>(out: &mut Output<W>, bytes: &[u8]) -> Result<(), String> {
    out.write_all(bytes).map_err(|err| err.to_string())
}
fn line<W: Write>(out: &mut Output<W>, value: &str) -> Result<(), String> {
    emit(out, value.as_bytes())?;
    emit(out, b"\n")
}
fn readable_name(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}
#[cfg(windows)]
fn windows_unsafe_component(name: &std::ffi::OsStr) -> bool {
    let text = name.to_string_lossy();
    let stem = text
        .trim_end_matches(&[' ', '.'][..])
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    text.contains(':')
        || text.ends_with(' ')
        || text.ends_with('.')
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}
fn checked_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\0') {
        return Err("empty or NUL-containing path".into());
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|p| !matches!(p, Component::Normal(_) | Component::CurDir))
    {
        return Err(
            "absolute paths and '..' are forbidden; use --root to select a directory".into(),
        );
    }
    #[cfg(windows)]
    if path.components().any(|component| match component {
        Component::Normal(name) => windows_unsafe_component(name),
        _ => false,
    }) {
        return Err("denied: unsafe Windows path normalization or device name".into());
    }
    if sensitive_path(path) {
        return Err("denied: sensitive path (default secret-file policy)".into());
    }
    Ok(path.to_path_buf())
}
fn single_path(value: &str) -> Result<PathBuf, String> {
    checked_path(value)
}

/// Reject static symlink aliases to sensitive paths (including directory
/// ancestors). A concurrently mutable tree still requires a trusted root;
/// this check is not an atomic anti-TOCTOU guarantee.
fn reject_symlink_components(root: &Dir, path: &Path, include_last: bool) -> Result<(), String> {
    let names: Vec<_> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let mut prefix = PathBuf::new();
    for (index, name) in names.iter().enumerate() {
        prefix.push(name);
        if !include_last && index + 1 == names.len() {
            break;
        }
        let metadata = root
            .symlink_metadata(&prefix)
            .map_err(|e| format!("cannot inspect path component: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err("denied: symlink traversal is not permitted".into());
        }
    }
    Ok(())
}
fn open_file(root: &Dir, value: &str) -> Result<cap_std::fs::File, String> {
    let path = single_path(value)?;
    reject_symlink_components(root, &path, true)?;
    let before = root
        .symlink_metadata(&path)
        .map_err(|e| format!("{value}: {e}"))?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(format!("{value}: only regular files are readable"));
    }
    if before.len() > MAX_FILE_BYTES as u64 {
        return Err(format!("{value}: exceeds 10 MiB file limit"));
    }
    // O_NONBLOCK keeps a raced-in FIFO from stalling during open(2).
    // O_NOFOLLOW atomically rejects a raced-in final symlink. cap-std
    // still confines every resolution to the operator-selected root.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let reader = {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        root.open_with(&path, &options)
    }
    .map_err(|e| format!("{value}: {e}"))?;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let reader = root.open(&path).map_err(|e| format!("{value}: {e}"))?;
    let opened_meta = reader.metadata().map_err(|e| e.to_string())?;
    if !opened_meta.is_file() {
        return Err(format!("{value}: not a regular file"));
    }
    if opened_meta.len() > MAX_FILE_BYTES as u64 {
        return Err(format!("{value}: exceeds 10 MiB file limit"));
    }
    // cap-std's Windows resolver confines the open to the selected root,
    // but its public OpenOptions API cannot pass OPEN_REPARSE_POINT. This
    // second name inspection catches some stable reparse replacements, not
    // an atomic concurrent-renaming race. Never use an ambient-path open.
    #[cfg(windows)]
    {
        let after = root
            .symlink_metadata(&path)
            .map_err(|e| format!("{value}: {e}"))?;
        if !after.is_file() || after.file_type().is_symlink() {
            return Err(format!("{value}: reparse or non-regular file denied"));
        }
    }
    Ok(reader)
}
fn file_bytes(root: &Dir, value: &str) -> Result<Vec<u8>, String> {
    let reader = open_file(root, value)?;
    let mut data = Vec::new();
    reader
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|e| format!("{value}: {e}"))?;
    if data.len() > MAX_FILE_BYTES {
        return Err(format!("{value}: exceeds 10 MiB file limit"));
    }
    Ok(data)
}
fn require_no_args(args: &[String], command: &str) -> Result<(), String> {
    if !args.is_empty() {
        Err(format!("{command}: unexpected argument"))
    } else {
        Ok(())
    }
}
fn reject_flags(value: &str) -> Result<(), String> {
    if value.starts_with('-') {
        Err(format!("unknown flag: {value}"))
    } else {
        Ok(())
    }
}
fn kind(meta: &Metadata) -> &'static str {
    if meta.file_type().is_symlink() {
        "link"
    } else if meta.is_file() {
        "file"
    } else if meta.is_dir() {
        "dir"
    } else {
        "special"
    }
}
fn list<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let (mut show_all, mut long) = (false, false);
    let mut target = ".";
    let mut seen_path = false;
    let mut options_ended = false;
    for arg in args {
        if !seen_path && !options_ended && arg == "--" {
            options_ended = true;
        } else if !seen_path && !options_ended && arg.starts_with('-') {
            for flag in arg[1..].chars() {
                match flag {
                    'a' => show_all = true,
                    'l' => long = true,
                    _ => return Err(format!("ls: unsupported flag -{flag}")),
                }
            }
        } else if seen_path {
            return Err("ls: expected at most one path".into());
        } else {
            target = arg;
            seen_path = true;
        }
    }
    let path = checked_path(target)?;
    reject_symlink_components(root, &path, false)?;
    let meta = root
        .symlink_metadata(&path)
        .map_err(|e| format!("{target}: {e}"))?;
    if !meta.is_dir() {
        if long {
            return line(
                out,
                &format!(
                    "{} {:>10} {}",
                    kind(&meta),
                    meta.len(),
                    readable_name(target)
                ),
            );
        }
        return line(out, &readable_name(target));
    }
    let mut entries = Vec::new();
    let mut scanned = 0usize;
    for entry in root.read_dir(&path).map_err(|e| format!("{target}: {e}"))? {
        scanned += 1;
        if scanned > MAX_DIRECTORY_ENTRIES {
            return Err("ls: more than 10000 entries".into());
        }
        // Entries can disappear while being enumerated. Do not abort the
        // entire listing because one raced or unreadable entry failed.
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        let display = name.to_string_lossy().to_string();
        // Even `ls -a` must not enumerate obviously sensitive names.
        if sensitive_name(&display) {
            continue;
        }
        if !show_all && display.starts_with('.') {
            continue;
        }
        let Ok(metadata) = root.symlink_metadata(path.join(&name)) else {
            continue;
        };
        entries.push((display, kind(&metadata), metadata.len()));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, file_type, len) in entries {
        if long {
            line(
                out,
                &format!("{file_type:<7} {len:>10} {}", readable_name(&name)),
            )?;
        } else {
            line(out, &readable_name(&name))?;
        }
    }
    Ok(())
}
fn cat<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    if args.is_empty() {
        return Err("cat: provide at least one file".into());
    }
    let (names, allow_dash) = match args.split_first() {
        Some((separator, names)) if separator == "--" => (names, true),
        _ => (args, false),
    };
    if names.is_empty() {
        return Err("cat: provide at least one file".into());
    }
    for name in names {
        if !allow_dash {
            reject_flags(name)?;
        }
        emit(out, &file_bytes(root, name)?)?;
    }
    Ok(())
}
fn numbered_args<'a>(command: &str, args: &'a [String]) -> Result<(usize, &'a str), String> {
    let (count, name, after_separator) = match args {
        [name] if name != "--" => (10, name.as_str(), false),
        [sep, name] if sep == "--" => (10, name.as_str(), true),
        [flag, count, name] if flag == "-n" && name != "--" => (
            count
                .parse::<usize>()
                .map_err(|_| format!("{command}: invalid line count"))?,
            name.as_str(),
            false,
        ),
        [flag, count, sep, name] if flag == "-n" && sep == "--" => (
            count
                .parse::<usize>()
                .map_err(|_| format!("{command}: invalid line count"))?,
            name.as_str(),
            true,
        ),
        _ => return Err(format!("{command}: expected [-n N] [--] FILE")),
    };
    if !after_separator {
        reject_flags(name)?;
    }
    if count > MAX_LINE_COUNT {
        return Err(format!("{command}: line count too large"));
    }
    Ok((count, name))
}
fn head_tail<W: Write>(
    root: &Dir,
    args: &[String],
    out: &mut Output<W>,
    last: bool,
) -> Result<(), String> {
    let command = if last { "tail" } else { "head" };
    let (count, name) = numbered_args(command, args)?;
    let file = open_file(root, name)?;
    if count == 0 {
        return Ok(());
    }
    // Head can stop at its requested line; tail keeps only its last N
    // lines, not the entire 10 MiB input. Both enforce the read cap even
    // if a concurrent writer grows a file after its size was checked.
    let mut reader = BufReader::new(file.take((MAX_FILE_BYTES + 1) as u64));
    let mut seen = 0usize;
    let mut total = 0usize;
    let mut trailing: VecDeque<Vec<u8>> = VecDeque::new();
    loop {
        let mut chunk = Vec::new();
        let size = reader
            .read_until(b'\n', &mut chunk)
            .map_err(|e| format!("{name}: {e}"))?;
        if size == 0 {
            break;
        }
        total += size;
        if total > MAX_FILE_BYTES {
            return Err(format!("{name}: exceeds 10 MiB file limit"));
        }
        seen += 1;
        if last {
            trailing.push_back(chunk);
            if trailing.len() > count {
                trailing.pop_front();
            }
        } else {
            emit(out, &chunk)?;
            if seen == count {
                break;
            }
        }
    }
    if last {
        for chunk in trailing {
            emit(out, &chunk)?;
        }
    }
    Ok(())
}
fn wc<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let (lines_only, name, after_separator) = match args {
        [name] if name != "--" => (false, name.as_str(), false),
        [sep, name] if sep == "--" => (false, name.as_str(), true),
        [flag, name] if flag == "-l" && name != "--" => (true, name.as_str(), false),
        [flag, sep, name] if flag == "-l" && sep == "--" => (true, name.as_str(), true),
        _ => return Err("wc: expected [-l] [--] FILE".into()),
    };
    if !after_separator {
        reject_flags(name)?;
    }
    let data = file_bytes(root, name)?;
    let count = data.iter().filter(|&&b| b == b'\n').count();
    if lines_only {
        line(out, &format!("{count} {}", readable_name(name)))
    } else {
        let words = data
            .split(u8::is_ascii_whitespace)
            .filter(|s| !s.is_empty())
            .count();
        line(
            out,
            &format!("{count} {words} {} {}", data.len(), readable_name(name)),
        )
    }
}
fn grep<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let mut index = 0;
    let mut numbers = false;
    let mut ignore_case = false;
    let mut options_ended = false;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "-n" => numbers = true,
            "-i" => ignore_case = true,
            "-ni" | "-in" => {
                numbers = true;
                ignore_case = true;
            }
            "--" => {
                index += 1;
                options_ended = true;
                break;
            }
            _ => break,
        }
        index += 1;
    }
    let operands = &args[index..];
    if operands.len() < 2 || operands.len() > 129 {
        return Err("grep: expected [-n] [-i] [--] TEXT FILE [FILE ...] (max 128 files)".into());
    }
    let pattern = &operands[0];
    // A leading dash is an option unless the caller explicitly used --.
    // Literal "-n" and "-i" patterns remain available after --.
    if !options_ended {
        reject_flags(pattern)?;
    }
    let files = &operands[1..];
    // Lowercase the whole string, not individual char windows: contextual
    // Unicode lowercasing (e.g. Greek final sigma) depends on neighbors.
    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern.clone()
    };
    for filename in files {
        if !options_ended {
            reject_flags(filename)?;
        }
        let file = open_file(root, filename)?;
        let mut reader = BufReader::new(file.take((MAX_FILE_BYTES + 1) as u64));
        let mut total = 0usize;
        let mut number = 0usize;
        loop {
            let mut value = Vec::new();
            let size = reader
                .read_until(b'\n', &mut value)
                .map_err(|e| format!("{filename}: {e}"))?;
            if size == 0 {
                break;
            }
            total += size;
            if total > MAX_FILE_BYTES {
                return Err(format!("{filename}: exceeds 10 MiB file limit"));
            }
            number += 1;
            let hay = String::from_utf8_lossy(&value);
            let hit = if ignore_case {
                hay.to_lowercase().contains(&needle)
            } else {
                hay.contains(&needle)
            };
            if hit {
                if files.len() > 1 {
                    emit(out, format!("{}:", readable_name(filename)).as_bytes())?;
                }
                if numbers {
                    emit(out, format!("{number}:").as_bytes())?;
                }
                emit(out, &value)?;
                if !value.ends_with(b"\n") {
                    emit(out, b"\n")?;
                }
            }
        }
    }
    Ok(())
}
fn stat<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let name = match args {
        [name] => {
            reject_flags(name)?;
            name.as_str()
        }
        [sep, name] if sep == "--" => name.as_str(),
        _ => return Err("stat: expected [--] FILE".into()),
    };
    let path = checked_path(name)?;
    reject_symlink_components(root, &path, false)?;
    let meta = root.symlink_metadata(path).map_err(|e| format!("{name}: {e}"))?;
    line(
        out,
        &format!(
            "{}: type={} size={}",
            readable_name(name),
            kind(&meta),
            meta.len()
        ),
    )
}
fn dispatch<W: Write>(
    root: &Dir,
    root_path: &Path,
    args: &[String],
    out: &mut Output<W>,
) -> Result<(), String> {
    let Some((command, rest)) = args.split_first() else {
        return Err("missing command (use --help)".into());
    };
    match command.as_str() {
        "pwd" => {
            require_no_args(rest, "pwd")?;
            line(out, &root_path.display().to_string())
        }
        "ls" => list(root, rest, out),
        "cat" => cat(root, rest, out),
        "head" => head_tail(root, rest, out, false),
        "tail" => head_tail(root, rest, out, true),
        "wc" => wc(root, rest, out),
        "grep" => grep(root, rest, out),
        "stat" => stat(root, rest, out),
        _ => Err(format!(
            "denied: command '{}' is not in the read-only allowlist",
            readable_name(command)
        )),
    }
}

/// Limit CPU consumption of a native invocation on Unix. This does not
/// prevent a blocking read on a slow or malicious filesystem (wall timeout
/// must be provided by the supervising client).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn enforce_cpu_budget() -> Result<(), String> {
    let limit = libc::rlimit {
        rlim_cur: 10 as libc::rlim_t,
        rlim_max: 11 as libc::rlim_t,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CPU, &limit) } != 0 {
        return Err(format!(
            "cannot install CPU time limit: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn enforce_read_only(root_path: &Path) -> Result<(), String> {
    use landlock::{
        Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd, Ruleset, RulesetAttr,
        RulesetCreatedAttr, RulesetStatus, ABI,
    };
    // ABI v3 includes TRUNCATE. A weaker ABI would silently allow truncation.
    let abi = ABI::V3;
    let builder = Ruleset::default().set_compatibility(CompatLevel::HardRequirement);
    let builder = builder
        .handle_access(AccessFs::from_all(abi))
        .map_err(|e| format!("Landlock policy: {e}"))?;
    let read_only = AccessFs::ReadFile | AccessFs::ReadDir;
    let rule = PathBeneath::new(PathFd::new(root_path).map_err(|e| e.to_string())?, read_only);
    let status = builder
        .create()
        .map_err(|e| format!("Landlock setup: {e}"))?
        .add_rule(rule)
        .map_err(|e| format!("Landlock read rule: {e}"))?
        .restrict_self()
        .map_err(|e| format!("Landlock enforcement: {e}"))?;
    if status.ruleset != RulesetStatus::FullyEnforced {
        return Err(format!(
            "Landlock is not fully enforced: {:?}",
            status.ruleset
        ));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn enforce_read_only() -> Result<(), String> {
    macos_guard::enforce_read_only()
}
#[cfg(target_os = "windows")]
fn enforce_read_only() -> Result<(), String> {
    windows_guard::enforce_read_only()
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn enforce_read_only() -> Result<(), String> {
    Err(
        "strict read-only sandbox requires Linux Landlock + seccomp, macOS Seatbelt or Windows restricted token"
            .into(),
    )
}

/// CLI entrypoint. No fallback is made if the OS sandbox cannot be enforced.
pub fn run(args: Vec<String>) -> Result<(), String> {
    // Parse every token before opening the root or executing any read.
    // A standalone informational flag is handled without OS sandbox setup.
    let (root, command) = match cli::parse(&args)? {
        cli::Request::Info(kind) => {
            print!("{}", kind.output());
            return Ok(());
        }
        cli::Request::Execute { root, command } => (root, command),
    };
    let root_path =
        std::fs::canonicalize(&root).map_err(|e| format!("cannot open root {root}: {e}"))?;
    if !root_path.is_dir() {
        return Err("--root must be a directory".into());
    }
    if sensitive_path(&root_path) {
        return Err("denied: selected root is a sensitive directory".into());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    enforce_cpu_budget()?;
    // On Windows, impersonate the write-restricted token BEFORE opening
    // any capability directory handles. Existing writable handles would
    // predate access checking and are not retroactively restricted.
    #[cfg(target_os = "windows")]
    enforce_read_only()?;
    let dir = Dir::open_ambient_dir(&root_path, ambient_authority())
        .map_err(|e| format!("cannot read root: {e}"))?;
    #[cfg(target_os = "linux")]
    enforce_read_only(&root_path)?;
    #[cfg(target_os = "macos")]
    enforce_read_only()?;
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    enforce_read_only()?;
    #[cfg(target_os = "linux")]
    metadata_guard::enforce_metadata_read_only()?;
    let stdout = std::io::stdout();
    let mut out = Output::new(stdout.lock());
    dispatch(&dir, &root_path, &command, &mut out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_fail_closed() {
        for value in ["/etc/passwd", "../outside", "src/../../outside", "", "x\0y"] {
            assert!(checked_path(value).is_err(), "{value:?}");
        }
        assert!(checked_path("src/lib.rs").is_ok());
        assert!(checked_path("./README.md").is_ok());
    }
    #[test]
    fn sensitive_paths_are_denied_by_default() {
        for path in [
            ".env",
            ".env.production",
            ".ssh/id_ed25519",
            ".aws/credentials",
            "nested/.git/config",
            "nested/TOKEN.JSON",
            "backup.tfstate.1",
            "private.pem",
            ".npmrc",
            "folder/secrets.json",
        ] {
            assert!(checked_path(path).is_err(), "{path:?}");
        }
        for path in ["README.md", ".env.example", ".env.sample", "src/main.rs"] {
            assert!(checked_path(path).is_ok(), "{path:?}");
        }
    }
    #[cfg(windows)]
    #[test]
    fn windows_path_aliases_are_denied() {
        for path in [
            r"C:\Windows\win.ini",
            r"\\server\share\file",
            r"folder\..\file",
            r"config.txt:alternate",
            "file.",
            "file ",
            "CON",
            "NUL.txt",
            "COM1",
        ] {
            assert!(checked_path(path).is_err(), "{path:?}");
        }
    }
    #[test]
    fn output_is_bounded() {
        let mut out = Output::new(Vec::new());
        assert!(emit(&mut out, &vec![b'a'; MAX_OUTPUT_BYTES]).is_ok());
        assert!(emit(&mut out, b"b").is_err());
    }
    #[test]
    fn unknown_command_denied() {
        let cwd = std::env::current_dir().unwrap();
        let root = Dir::open_ambient_dir(&cwd, ambient_authority()).unwrap();
        let mut out = Output::new(Vec::new());
        assert!(
            dispatch(&root, &cwd, &["rm".into(), "file".into()], &mut out)
                .unwrap_err()
                .contains("denied")
        );
        assert!(dispatch(
            &root,
            &cwd,
            &["sh".into(), "-c".into(), "echo".into()],
            &mut out
        )
        .unwrap_err()
        .contains("denied"));
    }
}
