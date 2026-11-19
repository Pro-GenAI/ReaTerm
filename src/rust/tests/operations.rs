//! Positive contract tests for every native read-only command.
//! All fixtures are private temporary directories; no host files are changed.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn call(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .arg("--root")
        .arg(root.path())
        .args(args)
        .output()
        .expect("launch native CLI")
}

fn success(root: &TempDir, args: &[&str]) -> String {
    let result = call(root, args);
    assert!(
        result.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).expect("UTF-8 test output")
}

#[test]
fn all_documented_read_commands_have_expected_outputs() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sample.txt"), "Alpha\nbeta gamma\n").unwrap();
    fs::create_dir(root.path().join("sub")).unwrap();

    assert_eq!(
        success(&root, &["pwd"]).trim(),
        fs::canonicalize(root.path()).unwrap().display().to_string()
    );
    assert_eq!(
        success(&root, &["cat", "sample.txt"]),
        "Alpha\nbeta gamma\n"
    );
    assert_eq!(
        success(&root, &["head", "-n", "1", "sample.txt"]),
        "Alpha\n"
    );
    assert_eq!(
        success(&root, &["tail", "-n", "1", "sample.txt"]),
        "beta gamma\n"
    );
    assert_eq!(
        success(&root, &["wc", "-l", "sample.txt"]),
        "2 sample.txt\n"
    );
    assert_eq!(success(&root, &["wc", "sample.txt"]), "2 3 17 sample.txt\n");
    assert_eq!(
        success(&root, &["grep", "-n", "beta", "sample.txt"]),
        "2:beta gamma\n"
    );
    assert_eq!(
        success(&root, &["grep", "-i", "ALPHA", "sample.txt"]),
        "Alpha\n"
    );
    assert_eq!(success(&root, &["grep", "missing", "sample.txt"]), "");
    assert_eq!(success(&root, &["ls", "sub"]), "");
    assert!(success(&root, &["stat", "sample.txt"]).contains("type=file size=17"));
    assert!(success(&root, &["stat", "sub"]).contains("type=dir size="));
}

#[test]
fn listing_is_sorted_and_hidden_entries_require_a_flag() {
    let root = tempfile::tempdir().unwrap();
    for filename in ["zeta", "alpha", ".hidden", "mid"] {
        fs::write(root.path().join(filename), "").unwrap();
    }
    assert_eq!(success(&root, &["ls"]), "alpha\nmid\nzeta\n");
    assert_eq!(success(&root, &["ls", "-a"]), ".hidden\nalpha\nmid\nzeta\n");
    assert_eq!(
        success(&root, &["ls", "-al"]),
        success(&root, &["ls", "-la"])
    );
    assert!(success(&root, &["ls", "-l", "alpha"]).contains("file"));
}

#[test]
fn file_reading_does_not_modify_contents_or_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("data");
    let data = b"one two\nthree four\n";
    fs::write(&path, data).unwrap();
    let before = fs::metadata(&path).unwrap();
    for args in [
        vec!["cat", "data"],
        vec!["wc", "data"],
        vec!["head", "data"],
        vec!["tail", "data"],
        vec!["grep", "one", "data"],
        vec!["stat", "data"],
        vec!["ls", "-l", "data"],
    ] {
        let _ = success(&root, &args);
    }
    let after = fs::metadata(&path).unwrap();
    assert_eq!(fs::read(path).unwrap(), data);
    assert_eq!(before.permissions().mode(), after.permissions().mode());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    // Access time is intentionally not asserted: reads can update atime.
}

#[test]
fn line_boundaries_empty_files_and_binary_data() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("no_newline"), "one\ntwo").unwrap();
    fs::write(root.path().join("empty"), b"").unwrap();
    fs::write(root.path().join("binary"), [0u8, 255, 10, 3]).unwrap();
    assert_eq!(success(&root, &["head", "-n", "0", "no_newline"]), "");
    assert_eq!(success(&root, &["tail", "-n", "0", "no_newline"]), "");
    assert_eq!(success(&root, &["head", "-n", "1", "no_newline"]), "one\n");
    assert_eq!(success(&root, &["tail", "-n", "1", "no_newline"]), "two");
    assert_eq!(
        success(&root, &["tail", "-n", "9", "no_newline"]),
        "one\ntwo"
    );
    assert_eq!(success(&root, &["cat", "empty"]), "");
    assert_eq!(success(&root, &["cat", "no_newline", "empty"]), "one\ntwo");
    assert_eq!(
        call(&root, &["cat", "binary"]).stdout,
        vec![0u8, 255, 10, 3]
    );
}

#[test]
fn unexpected_shell_metacharacters_are_literal_filenames() {
    let root = tempfile::tempdir().unwrap();
    let names = [
        "spaces are valid.txt",
        "semi;touch PWNED",
        "$(touch PWNED)",
        "a|tee PWNED",
        "quotation's.txt",
        "unicode-π.txt",
    ];
    for name in names {
        fs::write(root.path().join(name), b"safe\n").unwrap();
        assert_eq!(success(&root, &["cat", name]), "safe\n");
    }
    assert!(!root.path().join("PWNED").exists());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), names.len());
}

#[test]
fn relative_nested_files_and_default_root_work() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("nested/level")).unwrap();
    fs::write(root.path().join("nested/level/readme"), "nested\n").unwrap();
    assert_eq!(
        success(&root, &["cat", "./nested/level/readme"]),
        "nested\n"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_reaterm"))
        .current_dir(root.path())
        .args(["cat", "nested/level/readme"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"nested\n");
}
