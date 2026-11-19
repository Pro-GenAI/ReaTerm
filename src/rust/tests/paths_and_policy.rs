//! Negative tests for parsing, path confinement, symlinks and file types.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::process::{Command, Output};
use tempfile::TempDir;

fn invoke(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .arg("--root")
        .arg(root.path())
        .args(args)
        .output()
        .unwrap()
}

fn deny(root: &TempDir, args: &[&str]) {
    let out = invoke(root, args);
    assert_eq!(
        out.status.code(),
        Some(2),
        "unexpected exit status for {args:?}"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("reaterm:"),
        "missing error for {args:?}"
    );
}

#[test]
fn rejects_wrong_arity_and_unsupported_options() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "content\n").unwrap();
    let denied: &[&[&str]] = &[
        &[],
        &["pwd", "extra"],
        &["ls", "-R"],
        &["ls", "--recursive"],
        &["ls", "-alR"],
        &["ls", "file", "file"],
        &["ls", "--", "file"],
        &["cat"],
        &["cat", "-n", "file"],
        &["head"],
        &["head", "-n", "NaN", "file"],
        &["head", "-n", "100001", "file"],
        &["head", "-n", "-1", "file"],
        &["head", "file", "extra"],
        &["tail", "-n", "NaN", "file"],
        &["tail", "-n", "100001", "file"],
        &["wc"],
        &["wc", "-c", "file"],
        &["grep"],
        &["grep", "-r", "content", "."],
        &["grep", "-e", "content", "file"],
        &["grep", "content"],
        &["grep", "content", "file", "file"],
        &["stat"],
        &["stat", "-f", "file"],
        &["stat", "file", "file"],
        &["unknown", "file"],
    ];
    for args in denied {
        deny(&root, args);
    }
}

#[test]
fn rejects_root_traversal_and_absolute_operands() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), "secret\n").unwrap();
    for path in [
        "../secret",
        "nested/../../secret",
        "/etc/passwd",
        "/",
        "a/..",
        "",
    ] {
        for command in ["cat", "stat", "ls"] {
            deny(&root, &[command, path]);
        }
    }
    assert!(outside.path().join("secret").exists());
}

#[test]
fn symlinks_and_special_files_are_not_readable() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(root.path().join("inside"), b"inside").unwrap();
    fs::write(outside.path().join("outside"), b"secret").unwrap();

    symlink(
        root.path().join("inside"),
        root.path().join("internal_link"),
    )
    .unwrap();
    symlink(
        outside.path().join("outside"),
        root.path().join("external_link"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join("escaped_dir")).unwrap();

    let _socket = UnixListener::bind(root.path().join("socket")).unwrap();
    fs::create_dir(root.path().join("directory")).unwrap();

    for name in ["internal_link", "external_link", "socket", "directory"] {
        for command in ["cat", "head", "tail", "wc", "grep"] {
            let args = if command == "grep" {
                vec![command, "a", name]
            } else {
                vec![command, name]
            };
            deny(&root, &args);
        }
    }
    deny(&root, &["cat", "escaped_dir/outside"]);
    deny(&root, &["ls", "escaped_dir/outside"]);
    deny(&root, &["stat", "escaped_dir/outside"]);

    // Link metadata is safe to inspect without following it.
    let st = invoke(&root, &["stat", "external_link"]);
    assert!(st.status.success());
    assert!(String::from_utf8_lossy(&st.stdout).contains("type=link"));
    assert_eq!(fs::read(outside.path().join("outside")).unwrap(), b"secret");
}

#[test]
fn malformed_arguments_do_not_crash_or_mutate_fixture() {
    let root = tempfile::tempdir().unwrap();
    let protected = root.path().join("protected");
    fs::write(&protected, "stay\n").unwrap();
    let baseline_mode = fs::metadata(&protected).unwrap().permissions().mode();
    let evil = [
        "file; rm -rf .",
        "&& rm protected",
        "| tee protected",
        "> protected",
        "$(rm protected)",
        "./../protected",
        "sub/../../../protected",
        "/tmp/../etc/passwd",
        "--root",
    ];
    for value in evil {
        deny(&root, &["cat", value]);
    }
    // NUL cannot be passed through execve; Rust CLI itself also rejects it.
    assert_eq!(fs::read(&protected).unwrap(), b"stay\n");
    assert_eq!(
        fs::metadata(&protected).unwrap().permissions().mode(),
        baseline_mode
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn root_and_help_errors_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_reaterm");
    let no_command = Command::new(exe)
        .arg("--root")
        .arg(root.path())
        .output()
        .unwrap();
    assert_eq!(no_command.status.code(), Some(2));

    let invalid_root = Command::new(exe)
        .args(["--root", "/definitely-not-present-reaterm-dir", "ls"])
        .output()
        .unwrap();
    assert_eq!(invalid_root.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid_root.stderr).contains("cannot open root"));

    for arg in ["--help", "--version"] {
        let out = Command::new(exe).arg(arg).output().unwrap();
        assert!(out.status.success());
    }
}
