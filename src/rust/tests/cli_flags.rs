//! CLI option contract: information-only flags MUST be exclusive.
//! These tests check actual process exit codes and stdout/stderr, and verify
//! a mixed informational flag prevents native dispatch before file reads.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

fn execute(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .args(args)
        .output()
        .expect("launch ReaTerm")
}

#[test]
fn information_only_flags_succeed_alone() {
    for (flag, expected) in [
        ("-h", "Usage:"),
        ("--help", "Usage:"),
        ("-V", env!("CARGO_PKG_VERSION")),
        ("--version", env!("CARGO_PKG_VERSION")),
        ("--list-commands", "cat\n"),
        ("--limits", "10 MiB"),
        ("--license", "MIT\n"),
        ("--about", "Landlock"),
    ] {
        let out = execute(&[flag]);
        assert!(out.status.success(), "{flag}: {:?}", out.stderr);
        assert!(out.stderr.is_empty(), "{flag}: unexpected stderr");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains(expected),
            "{flag}: missing {expected}"
        );
    }
    assert_eq!(execute(&["--version"]).stdout, execute(&["-V"]).stdout);
    assert_eq!(execute(&["--help"]).stdout, execute(&["-h"]).stdout);
}

#[test]
fn informational_flags_combined_with_commands_or_other_flags_error_first() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("canary");
    fs::write(&path, b"SHOULD_NOT_APPEAR_IN_STDOUT\n").unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let directory = root.path().to_str().unwrap();

    let attempts: &[&[&str]] = &[
        &["--help", "cat", "canary"],
        &["cat", "canary", "--help"],
        &["cat", "missing", "-h"],
        &["--version", "cat", "canary"],
        &["cat", "canary", "--version"],
        &["-V", "ls"],
        &["ls", "-V"],
        &["--list-commands", "ls"],
        &["ls", "--list-commands"],
        &["--limits", "cat", "canary"],
        &["cat", "canary", "--limits"],
        &["--license", "chmod", "777", "canary"],
        &["--about", "rm", "canary"],
        &["-h", "--version"],
        &["--version", "--help"],
        &["--license", "--license"],
        &["--root", directory, "--help", "cat", "canary"],
        &["--help", "--root", directory, "cat", "canary"],
        &["-C", directory, "--version", "cat", "canary"],
        &["--root=.", "--about", "ls"],
        &[
            "--root",
            "/definitely-missing-reatrm-root",
            "--help",
            "cat",
            "canary",
        ],
    ];

    for args in attempts {
        let out = execute(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            out.stdout.is_empty(),
            "command or info output leaked for {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("must be used alone"),
            "missing exclusive informational option diagnostic for {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(fs::read(&path).unwrap(), b"SHOULD_NOT_APPEAR_IN_STDOUT\n");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode(), mode);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn information_flag_after_missing_file_is_rejected_before_read() {
    let root = tempfile::tempdir().unwrap();
    let root_str = root.path().to_str().unwrap();
    for flag in [
        "--help",
        "-h",
        "--version",
        "-V",
        "--list-commands",
        "--limits",
        "--license",
        "--about",
    ] {
        let out = execute(&["--root", root_str, "cat", "does-not-exist", flag]);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("must be used alone") && !stderr.contains("No such file"),
            "{flag}: {stderr}"
        );
    }
}

#[test]
fn root_options_are_execution_only_and_precede_commands() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("safe.txt"), "read only\n").unwrap();
    let dir = root.path().to_str().unwrap();
    let equal_form = format!("--root={dir}");

    for args in [
        vec!["--root", dir, "cat", "safe.txt"],
        vec!["-C", dir, "cat", "safe.txt"],
        vec![equal_form.as_str(), "cat", "safe.txt"],
    ] {
        let out = execute(&args);
        assert!(out.status.success(), "{args:?}: {:?}", out.stderr);
        assert_eq!(out.stdout, b"read only\n");
    }

    for args in [
        vec!["cat", "safe.txt", "--root", dir],
        vec!["cat", "safe.txt", "-C", dir],
        vec!["cat", "safe.txt", equal_form.as_str()],
        vec!["--root", dir, "--root", dir, "cat", "safe.txt"],
        vec!["-C", dir, "--root", dir, "cat", "safe.txt"],
        vec!["--root", dir],
        vec!["--root="],
        vec!["--invalid", "cat", "safe.txt"],
    ] {
        let out = execute(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "should not read file: {args:?}");
    }
    assert_eq!(
        fs::read(root.path().join("safe.txt")).unwrap(),
        b"read only\n"
    );
}

#[test]
fn help_and_version_require_no_filesystem_access() {
    let root = tempfile::tempdir().unwrap();
    let bogus = root.path().join("nonexistent");
    let bogus = bogus.to_str().unwrap();
    for flag in ["--help", "--version", "--limits", "--license"] {
        let out = execute(&[flag]);
        assert!(out.status.success());
        let invalid_mixed = execute(&["--root", bogus, flag, "cat", "missing"]);
        assert_eq!(invalid_mixed.status.code(), Some(2));
        assert!(invalid_mixed.stdout.is_empty());
        assert!(String::from_utf8_lossy(&invalid_mixed.stderr).contains("must be used alone"));
    }
}
