//! Windows native executable integration tests.
//! All adversarial operands target disposable fixtures under a private root.
#![cfg(windows)]

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn run(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .args(["--root", root.path().to_str().unwrap()])
        .args(args)
        .output()
        .expect("launch native Windows CLI")
}

#[test]
fn windows_supported_native_commands_read_without_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let sample = root.path().join("canary.txt");
    fs::write(&sample, b"Alpha\nbeta gamma\n").unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    let before = fs::metadata(&sample).unwrap();

    for (argv, expected) in [
        (vec!["cat", "canary.txt"], "Alpha\nbeta gamma\n"),
        (vec!["head", "-n", "1", "canary.txt"], "Alpha\n"),
        (vec!["tail", "-n", "1", "canary.txt"], "beta gamma\n"),
        (vec!["wc", "-l", "canary.txt"], "2 canary.txt\n"),
        (vec!["grep", "-n", "beta", "canary.txt"], "2:beta gamma\n"),
    ] {
        let output = run(&root, &argv);
        assert!(
            output.status.success(),
            "{argv:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected.as_bytes(), "{argv:?}");
    }
    for argv in [
        vec!["pwd"],
        vec!["ls"],
        vec!["ls", "-la"],
        vec!["stat", "canary.txt"],
    ] {
        let output = run(&root, &argv);
        assert!(
            output.status.success(),
            "{argv:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read(&sample).unwrap(), b"Alpha\nbeta gamma\n");
    assert_eq!(fs::metadata(&sample).unwrap().len(), before.len());
    assert_eq!(
        fs::metadata(&sample).unwrap().modified().unwrap(),
        before.modified().unwrap()
    );
}

#[test]
fn windows_destructive_commands_are_denied_and_fixture_untouched() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("canary.txt");
    fs::write(&file, b"protected\n").unwrap();
    let original = fs::metadata(&file).unwrap();
    let original_names = fs::read_dir(root.path()).unwrap().count();

    for command in [
        "rm",
        "rmdir",
        "mv",
        "cp",
        "del",
        "erase",
        "copy",
        "move",
        "rename",
        "ren",
        "mkdir",
        "md",
        "attrib",
        "icacls",
        "takeown",
        "chmod",
        "chown",
        "touch",
        "truncate",
        "tee",
        "cmd",
        "powershell",
        "pwsh",
        "python",
        "python3",
        "node",
        "bash",
        "sh",
        "git",
    ] {
        let output = run(&root, &[command, "canary.txt"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{command} unexpectedly allowed"
        );
        assert!(output.stdout.is_empty(), "{command} printed stdout");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("denied"),
            "{command} did not report a denial"
        );
    }
    for args in [
        vec!["cat", "../outside"],
        vec!["cat", "C:\\Windows\\win.ini"],
        vec!["cat", "\\\\server\\share\\secret"],
        vec!["ls", "-R"],
        vec!["cat", "canary.txt", "--help"],
        vec!["--help", "cat", "canary.txt"],
        vec!["--version", "rm", "canary.txt"],
    ] {
        let output = run(&root, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
    assert_eq!(fs::read(&file).unwrap(), b"protected\n");
    let after = fs::metadata(&file).unwrap();
    assert_eq!(after.len(), original.len());
    assert_eq!(after.modified().unwrap(), original.modified().unwrap());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), original_names);
}

#[test]
fn windows_info_flags_work_without_kernel_sandbox() {
    let binary = env!("CARGO_BIN_EXE_reaterm");
    for flag in [
        "--help",
        "-h",
        "--version",
        "-V",
        "--limits",
        "--about",
        "--license",
        "--list-commands",
    ] {
        let result = Command::new(binary).arg(flag).output().unwrap();
        assert!(
            result.status.success(),
            "{flag}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!result.stdout.is_empty(), "{flag}: no informational output");
    }
}

#[test]
fn windows_filename_metacharacters_are_literal_not_shell_syntax() {
    let root = tempfile::tempdir().unwrap();
    let name = "literal;echo MALICIOUS.txt";
    fs::write(root.path().join(name), "safe\n").unwrap();
    let result = run(&root, &["cat", name]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"safe\n");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn windows_escape_via_symlink_does_not_read_outside_root() {
    use std::os::windows::fs::symlink_file;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("sensitive.txt");
    fs::write(&target, b"PRIVATE_CONTENT\n").unwrap();
    let link = root.path().join("link");
    // User accounts lacking Developer Mode or SeCreateSymbolicLinkPrivilege
    // cannot create symlinks. Skip only this case; other tests still run.
    if let Err(err) = symlink_file(&target, &link) {
        eprintln!("symlink test skipped: {err}");
        return;
    }
    let result = run(&root, &["cat", "link"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&result.stdout).contains("PRIVATE_CONTENT"));
    assert_eq!(fs::read(&target).unwrap(), b"PRIVATE_CONTENT\n");
}
