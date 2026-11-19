//! Cross-platform adversarial command tests against the real native CLI.
//! Only disposable fixture trees are used; success never means executing a shell.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("start native ReaTerm")
}

fn denied(root: &Path, arguments: &[&str]) {
    let result = run(root, arguments);
    assert_eq!(result.status.code(), Some(2), "{arguments:?}: {result:?}");
    assert!(result.stdout.is_empty(), "stdout leaked for {arguments:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("reaterm:"),
        "no denial diagnostic for {arguments:?}"
    );
}

#[test]
fn every_file_reader_denies_absolute_and_parent_traversal_without_disclosure() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let safe_file = root.path().join("safe.txt");
    let secret_file = outside.path().join("secret.txt");
    fs::write(&safe_file, b"unchanged\n").unwrap();
    fs::write(&secret_file, b"DO_NOT_EXPOSE\n").unwrap();
    let before = fs::metadata(&safe_file).unwrap();
    let escape = format!(
        "../{}/secret.txt",
        outside.path().file_name().unwrap().to_string_lossy()
    );
    let absolute = secret_file.to_str().unwrap();

    for path in [escape.as_str(), absolute] {
        for args in [
            vec!["ls", path],
            vec!["cat", path],
            vec!["head", path],
            vec!["tail", path],
            vec!["wc", path],
            vec!["grep", "DO_NOT_EXPOSE", path],
            vec!["stat", path],
        ] {
            denied(root.path(), &args);
        }
    }
    let after = fs::metadata(&safe_file).unwrap();
    assert_eq!(fs::read(&safe_file).unwrap(), b"unchanged\n");
    assert_eq!(fs::read(&secret_file).unwrap(), b"DO_NOT_EXPOSE\n");
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(before.permissions().readonly(), after.permissions().readonly());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn shell_interpreters_and_mutation_commands_never_dispatch() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("canary.txt");
    fs::write(&file, b"STILL_INTACT\n").unwrap();
    let before = fs::metadata(&file).unwrap();
    for args in [
        vec!["sh", "-c", "echo corrupt > canary.txt"],
        vec!["bash", "-c", "touch PWNED"],
        vec!["python3", "-c", "open('PWNED','w')"],
        vec!["node", "-e", "require('fs').writeFileSync('PWNED','x')"],
        vec!["rm", "canary.txt"],
        vec!["chmod", "777", "canary.txt"],
        vec!["touch", "PWNED"],
        vec!["cat", "canary.txt", "--version"],
    ] {
        denied(root.path(), &args);
    }
    let after = fs::metadata(&file).unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"STILL_INTACT\n");
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn informational_flags_are_rejected_before_invalid_root_resolution() {
    let root = tempfile::tempdir().unwrap();
    let not_a_root = root.path().join("missing");
    let missing = not_a_root.to_str().unwrap();
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
        for args in [
            vec!["--root", missing, "cat", "secret.txt", flag],
            vec!["--root", missing, flag, "cat", "secret.txt"],
        ] {
            let result = run(root.path(), &args);
            assert_eq!(result.status.code(), Some(2), "{args:?}");
            assert!(result.stdout.is_empty(), "{args:?}");
            let stderr = String::from_utf8_lossy(&result.stderr);
            assert!(stderr.contains("must be used alone"), "{args:?}: {stderr}");
            assert!(!stderr.contains("cannot open root"), "{args:?}: {stderr}");
        }
    }
}
