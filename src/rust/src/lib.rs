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
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 10_000;
const MAX_LINE_COUNT: usize = 100_000;

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
    Ok(path.to_path_buf())
}
fn single_path(value: &str) -> Result<PathBuf, String> {
    checked_path(value)
}
fn file_bytes(root: &Dir, value: &str) -> Result<Vec<u8>, String> {
    let path = single_path(value)?;
    let before = root
        .symlink_metadata(&path)
        .map_err(|e| format!("{value}: {e}"))?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(format!("{value}: only regular files are readable"));
    }
    if before.len() > MAX_FILE_BYTES as u64 {
        return Err(format!("{value}: exceeds 10 MiB file limit"));
    }
    let reader = root.open(&path).map_err(|e| format!("{value}: {e}"))?;
    if !reader.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{value}: not a regular file"));
    }
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
fn filename_parts(args: &[String]) -> Result<(&str, &[String]), String> {
    args.split_first()
        .map(|(name, rest)| (name.as_str(), rest))
        .ok_or_else(|| "missing file operand".to_string())
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
    for arg in args {
        if !seen_path && arg.starts_with('-') {
            if arg == "--" {
                return Err("'--' is not supported".into());
            }
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
    for entry in root.read_dir(&path).map_err(|e| format!("{target}: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let display = name.to_string_lossy().to_string();
        if !show_all && display.starts_with('.') {
            continue;
        }
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err("ls: more than 10000 entries".into());
        }
        let metadata = root
            .symlink_metadata(path.join(&name))
            .map_err(|e| format!("{}: {e}", display))?;
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
    for name in args {
        reject_flags(name)?;
        emit(out, &file_bytes(root, name)?)?;
    }
    Ok(())
}
fn numbered_args<'a>(command: &str, args: &'a [String]) -> Result<(usize, &'a str), String> {
    let (count, name) = match args {
        [name] => (10, name.as_str()),
        [flag, count, name] if flag == "-n" => (
            count
                .parse::<usize>()
                .map_err(|_| format!("{command}: invalid line count"))?,
            name.as_str(),
        ),
        _ => return Err(format!("{command}: expected [-n N] FILE")),
    };
    reject_flags(name)?;
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
    let bytes = file_bytes(root, name)?;
    if count == 0 {
        return Ok(());
    }
    let lines: Vec<&[u8]> = bytes.split_inclusive(|b| *b == b'\n').collect();
    let lines = if last {
        &lines[lines.len().saturating_sub(count)..]
    } else {
        &lines[..lines.len().min(count)]
    };
    for part in lines {
        emit(out, part)?;
    }
    Ok(())
}
fn wc<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let (lines_only, name) = match args {
        [name] => (false, name.as_str()),
        [flag, name] if flag == "-l" => (true, name.as_str()),
        _ => return Err("wc: expected [-l] FILE".into()),
    };
    reject_flags(name)?;
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
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "-n" => numbers = true,
            "-i" => ignore_case = true,
            "-ni" | "-in" => {
                numbers = true;
                ignore_case = true;
            }
            _ => break,
        }
        index += 1;
    }
    if args.len() != index + 2 {
        return Err("grep: expected [-n] [-i] TEXT FILE".into());
    }
    let pattern = &args[index];
    let file = &args[index + 1];
    reject_flags(file)?;
    let content = file_bytes(root, file)?;
    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern.clone()
    };
    for (number, value) in content.split_inclusive(|b| *b == b'\n').enumerate() {
        let hay = String::from_utf8_lossy(value);
        let hit = if ignore_case {
            hay.to_lowercase().contains(&needle)
        } else {
            hay.contains(&needle)
        };
        if hit {
            if numbers {
                emit(out, format!("{}:", number + 1).as_bytes())?;
            }
            emit(out, value)?;
            if !value.ends_with(b"\n") {
                emit(out, b"\n")?;
            }
        }
    }
    Ok(())
}
fn stat<W: Write>(root: &Dir, args: &[String], out: &mut Output<W>) -> Result<(), String> {
    let (name, rest) = filename_parts(args)?;
    require_no_args(rest, "stat")?;
    reject_flags(name)?;
    let meta = root
        .symlink_metadata(checked_path(name)?)
        .map_err(|e| format!("{name}: {e}"))?;
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

#[cfg(target_os = "linux")]
fn enforce_read_only() -> Result<(), String> {
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
    let rule = PathBeneath::new(PathFd::new("/").map_err(|e| e.to_string())?, read_only);
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
    Err("strict read-only sandbox requires Linux Landlock + seccomp, macOS Seatbelt or Windows restricted token".into())
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
    // On Windows, impersonate the write-restricted token BEFORE opening
    // any capability directory handles. Existing writable handles would
    // predate access checking and are not retroactively restricted.
    #[cfg(target_os = "windows")]
    enforce_read_only()?;
    let dir = Dir::open_ambient_dir(&root_path, ambient_authority())
        .map_err(|e| format!("cannot read root: {e}"))?;
    #[cfg(not(target_os = "windows"))]
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
