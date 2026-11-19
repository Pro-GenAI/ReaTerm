//! Conservative, fully validated CLI argument parsing.
//!
//! Informational flags are **exclusive**: they may appear only as the
//! single argument. The entire argv is checked before filesystem access,
//! Landlock/seccomp setup, or native command dispatch.

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Info {
    Help,
    Version,
    Commands,
    Limits,
    License,
    About,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Request {
    Info(Info),
    Execute { root: String, command: Vec<String> },
}

const COMMANDS: &str = "pwd\nls\ncat\nhead\ntail\nwc\ngrep\nstat\n";
const LIMITS: &str =
    "Maximum file size: 10 MiB\nMaximum stdout: 1 MiB\nMaximum ls entries: 10000\nMaximum requested head/tail lines: 100000\n";
const HELP: &str = "\
ReaTerm - native read-only terminal for Linux, macOS, and Windows\n\
\n\
Usage:\n\
  reaterm [--root DIRECTORY] COMMAND [ARG...]\n\
  reaterm -C DIRECTORY COMMAND [ARG...]\n\
  reaterm --help | --version | --list-commands | --limits | --license | --about\n\
\n\
Informational flags (must be used ALONE, never with a command):\n\
  -h, --help          Show this help\n\
  -V, --version       Show the compiled binary version\n\
  --list-commands     List the supported read-only commands\n\
  --limits            Show resource limits\n\
  --license           Show the license\n\
  --about             Show platform and security requirements\n\
\n\
Execution options (must precede COMMAND):\n\
  --root DIR          Scope filesystem reads to DIR (default: current dir)\n\
  --root=DIR          Equivalent form\n\
  -C DIR              Short form for --root DIR\n\
\n\
Read-only commands:\n\
  pwd                       Print configured root\n\
  ls [-a] [-l] [PATH]       List directory contents\n\
  cat FILE [FILE...]        Print regular files\n\
  head [-n N] FILE          Print first N lines\n\
  tail [-n N] FILE          Print last N lines\n\
  wc [-l] FILE              Count lines, words and bytes\n\
  grep [-n] [-i] TEXT FILE  Search for a literal string\n\
  stat PATH                 Show path type and size\n\
\n\
Examples:\n\
  reaterm ls -la\n\
  reaterm cat README.md\n\
  reaterm --root ./project grep -n TODO README.md\n\
  reaterm --help\n\
\n\
Info flags cannot be combined with other flags, arguments, or commands.\n\
No shell, external commands, filesystem writes or unknown flags are allowed.\n";

impl Info {
    fn from_arg(arg: &str) -> Option<Self> {
        Some(match arg {
            "-h" | "--help" => Self::Help,
            "-V" | "--version" => Self::Version,
            "--list-commands" => Self::Commands,
            "--limits" => Self::Limits,
            "--license" => Self::License,
            "--about" => Self::About,
            _ => return None,
        })
    }

    pub(crate) fn output(&self) -> String {
        match self {
            Self::Help => HELP.to_string(),
            Self::Version => format!("reaterm {}\n", env!("CARGO_PKG_VERSION")),
            Self::Commands => COMMANDS.to_string(),
            Self::Limits => LIMITS.to_string(),
            Self::License => "MIT\n".to_string(),
            Self::About => format!(
                "ReaTerm {}\nPlatforms: Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64\nSecurity: Linux Landlock + seccomp, macOS Seatbelt, Windows restricted access token\nNo external commands or writes\n",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }
}

/// Recognize a reserved flag anywhere in argv BEFORE executing commands.
/// This includes flags after the command name: e.g. cat FILE --version
/// must *not* read FILE and then discover the unexpected flag.
pub(crate) fn parse(args: &[String]) -> Result<Request, String> {
    if let Some((flag, kind)) = args
        .iter()
        .find_map(|value| Info::from_arg(value).map(|kind| (value, kind)))
    {
        if args.len() != 1 {
            return Err(format!(
                "informational flag {flag} must be used alone; do not combine it with a command, --root, or other flags"
            ));
        }
        return Ok(Request::Info(kind));
    }

    let mut root = String::from(".");
    let mut root_seen = false;
    let mut index = 0;
    while let Some(token) = args.get(index) {
        if matches!(token.as_str(), "--root" | "-C") || token.starts_with("--root=") {
            if root_seen {
                return Err("root option may only be specified once".into());
            }
            root_seen = true;
            if let Some(value) = token.strip_prefix("--root=") {
                if value.is_empty() {
                    return Err("--root= requires a nonempty directory".into());
                }
                root = value.to_string();
            } else {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("missing directory for {token}"))?;
                if value.is_empty() || value.starts_with('-') {
                    return Err(format!("{token} requires a directory operand"));
                }
                root = value.clone();
            }
            index += 1;
        } else if token.starts_with('-') {
            return Err(format!("unknown global option: {token}"));
        } else {
            break;
        }
    }

    if index >= args.len() {
        return Err("missing command (use --help)".into());
    }
    let command = args[index..].to_vec();
    // Explicitly reserve root selectors at any position. They must not
    // be accepted as operands after a command starts.
    if command
        .iter()
        .skip(1)
        .any(|arg| arg == "--root" || arg == "-C" || arg.starts_with("--root="))
    {
        return Err("--root and -C must appear before the command".into());
    }
    Ok(Request::Execute { root, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &[&str]) -> Result<Request, String> {
        parse(
            &words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn informational_options_work_only_alone() {
        for flag in [
            "-h",
            "--help",
            "-V",
            "--version",
            "--list-commands",
            "--limits",
            "--license",
            "--about",
        ] {
            assert!(matches!(parse_words(&[flag]), Ok(Request::Info(_))));
            for args in [
                vec![flag, "cat", "anything"],
                vec!["cat", "anything", flag],
                vec!["ls", flag],
                vec![flag, "--root", "."],
                vec!["--root", ".", flag],
                vec![flag, "--version"],
            ] {
                assert!(
                    parse_words(&args)
                        .unwrap_err()
                        .contains("must be used alone"),
                    "flag {flag} was not exclusive for {args:?}"
                );
            }
        }
    }

    #[test]
    fn execution_options_are_before_commands_only() {
        assert_eq!(
            parse_words(&["ls"]).unwrap(),
            Request::Execute {
                root: ".".into(),
                command: vec!["ls".into()]
            }
        );
        for args in [
            vec!["--root", "workspace", "cat", "README.md"],
            vec!["--root=workspace", "cat", "README.md"],
            vec!["-C", "workspace", "cat", "README.md"],
        ] {
            assert_eq!(
                parse_words(&args).unwrap(),
                Request::Execute {
                    root: "workspace".into(),
                    command: vec!["cat".into(), "README.md".into()]
                }
            );
        }
        for args in [
            vec!["--root"],
            vec!["-C"],
            vec!["--root="],
            vec!["--root", "workspace"],
            vec!["--root", "a", "--root", "b", "ls"],
            vec!["-C", "a", "--root=b", "ls"],
            vec!["cat", "file", "--root", "x"],
            vec!["ls", "-C", "x"],
            vec!["ls", "--root=x"],
            vec!["--unknown", "ls"],
            vec!["-hV"],
        ] {
            assert!(parse_words(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn info_request_cannot_attempt_root_resolution() {
        let args = ["--root", "/definitely-missing", "--help", "cat", "data"];
        assert!(parse_words(&args)
            .unwrap_err()
            .contains("must be used alone"));
    }

    #[test]
    fn static_information_documents_security_boundary() {
        assert!(Info::Help.output().contains("must be used ALONE"));
        assert!(Info::Limits.output().contains("10 MiB"));
        assert_eq!(Info::License.output(), "MIT\n");
        assert!(Info::Commands.output().lines().any(|line| line == "cat"));
        assert!(Info::About.output().contains("seccomp"));
    }
}
