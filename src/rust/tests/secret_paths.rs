//! Default confidentiality deny rules are a defense-in-depth guard only.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn invoke(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("run native CLI")
}

fn denied(root: &Path, args: &[&str]) {
    let output = invoke(root, args);
    assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    assert!(output.stdout.is_empty(), "unexpected stdout for {args:?}");
}

#[test]
fn common_secret_files_and_directories_are_denied() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".ssh")).unwrap();
    fs::write(root.path().join(".ssh").join("config"), b"PRIVATE\n").unwrap();
    fs::write(root.path().join(".env.production"), b"TOKEN=PRIVATE\n").unwrap();
    fs::write(root.path().join("credentials.json"), b"PRIVATE\n").unwrap();
    fs::write(root.path().join("backup.tfstate"), b"PRIVATE\n").unwrap();
    for name in [
        ".git-credentials",
        ".pgpass",
        ".my.cnf",
        "kubeconfig",
        "client.jks",
        "vault.keystore",
        "vault.kdbx",
        "ssh.ppk",
    ] {
        fs::write(root.path().join(name), b"PRIVATE\n").unwrap();
    }
    fs::write(root.path().join("safe.txt"), b"PUBLIC\n").unwrap();
    fs::write(root.path().join(".env.example"), b"EXAMPLE\n").unwrap();

    for name in [
        ".env.production",
        ".ssh/config",
        "credentials.json",
        "backup.tfstate",
        ".git-credentials",
        ".pgpass",
        ".my.cnf",
        "kubeconfig",
        "client.jks",
        "vault.keystore",
        "vault.kdbx",
        "ssh.ppk",
    ] {
        for args in [
            vec!["cat", name],
            vec!["head", name],
            vec!["tail", name],
            vec!["wc", name],
            vec!["grep", "PRIVATE", name],
            vec!["stat", name],
            vec!["ls", name],
        ] {
            denied(root.path(), &args);
        }
    }
    for args in [vec!["ls", ".ssh"], vec!["stat", ".ssh"]] {
        denied(root.path(), &args);
    }
    let listing = invoke(root.path(), &["ls", "-a"]);
    assert!(listing.status.success(), "{listing:?}");
    let names = String::from_utf8_lossy(&listing.stdout);
    assert!(names.contains("safe.txt"));
    assert!(names.contains(".env.example"));
    for hidden in [
        ".env.production",
        ".ssh",
        "credentials.json",
        "backup.tfstate",
        ".git-credentials",
        ".pgpass",
        ".my.cnf",
        "kubeconfig",
        "client.jks",
        "vault.keystore",
        "vault.kdbx",
        "ssh.ppk",
    ] {
        assert!(!names.contains(hidden), "leaked name: {hidden}");
    }
    assert_eq!(invoke(root.path(), &["cat", "safe.txt"]).stdout, b"PUBLIC\n");
    assert_eq!(invoke(root.path(), &["cat", ".env.example"]).stdout, b"EXAMPLE\n");
}

#[test]
fn a_sensitive_directory_cannot_be_selected_as_root() {
    let root = tempfile::tempdir().unwrap();
    let secret = root.path().join(".ssh");
    fs::create_dir(&secret).unwrap();
    denied(&secret, &["ls"]);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn symlink_alias_cannot_bypass_secret_directory_filter() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".ssh")).unwrap();
    fs::write(root.path().join(".ssh").join("config"), b"PRIVATE\n").unwrap();
    fs::write(root.path().join(".ssh").join("other"), b"PRIVATE\n").unwrap();
    symlink(".ssh", root.path().join("alias")).unwrap();
    denied(root.path(), &["cat", "alias/other"]);
    denied(root.path(), &["grep", "PRIVATE", "alias/other"]);
    denied(root.path(), &["ls", "alias/other"]);
    denied(root.path(), &["stat", "alias/other"]);
}
