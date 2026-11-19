//! Exercising real CLI enforcement against disposable file fixtures.
//! All attempted mutations must be rejected before any filesystem changes.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn only_read_only_commands_are_executable() {
    let root = tempfile::tempdir().expect("disposable fixture root");
    let protected = root.path().join("protected.txt");
    fs::write(&protected, "original content\n").unwrap();
    let mode = fs::metadata(&protected).unwrap().permissions().mode();
    let binary = env!("CARGO_BIN_EXE_reaterm");

    let read = Command::new(binary)
        .args([
            "--root",
            root.path().to_str().unwrap(),
            "cat",
            "protected.txt",
        ])
        .output()
        .expect("spawn ReaTerm");
    assert!(
        read.status.success(),
        "read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert_eq!(read.stdout, b"original content\n");

    let rejected: &[&[&str]] = &[
        &["rm", "protected.txt"],
        &["rmdir", "."],
        &["mv", "protected.txt", "renamed.txt"],
        &["cp", "protected.txt", "copied.txt"],
        &["touch", "created.txt"],
        &["mkdir", "created"],
        &["chmod", "000", "protected.txt"],
        &["chown", "root", "protected.txt"],
        &["chgrp", "root", "protected.txt"],
        &["truncate", "-s", "0", "protected.txt"],
        &["ln", "protected.txt", "alias"],
        &["install", "protected.txt", "installed"],
        &["tee", "protected.txt"],
        &["dd", "of=protected.txt"],
        &["sed", "-i", "s/original/changed/", "protected.txt"],
        &["find", ".", "-delete"],
        &["git", "checkout", "--", "protected.txt"],
        &["bash", "-c", "echo broken > protected.txt"],
        &["sh", "-c", "echo broken > protected.txt"],
        &["python3", "-c", "open('protected.txt', 'w').close()"],
        &["ls", "-R"],
        &["cat", "--output=protected.txt"],
        &["grep", "-r", "original", "."],
        &["cat", "../protected.txt"],
        &["cat", "/etc/passwd"],
    ];
    for attempt in rejected {
        let output = Command::new(binary)
            .arg("--root")
            .arg(root.path())
            .args(*attempt)
            .output()
            .expect("spawn ReaTerm");
        assert!(
            !output.status.success(),
            "unexpectedly permitted: {:?}",
            attempt
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("reaterm:"),
            "missing denial diagnostic: {:?}",
            attempt
        );
    }
    assert_eq!(fs::read(&protected).unwrap(), b"original content\n");
    assert_eq!(fs::metadata(&protected).unwrap().permissions().mode(), mode);
    let entries: Vec<_> = fs::read_dir(root.path()).unwrap().collect();
    assert_eq!(entries.len(), 1, "unexpected file creation inside fixture");
}
