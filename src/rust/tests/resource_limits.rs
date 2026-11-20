//! Resource-boundary tests. Large-directory stress test is opt-in.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
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

#[test]
fn one_megabyte_output_is_allowed_but_one_byte_more_is_denied() {
    let root = tempfile::tempdir().unwrap();
    let output_limit = 1024 * 1024;
    fs::write(root.path().join("exact"), vec![b'x'; output_limit]).unwrap();
    fs::write(root.path().join("over"), vec![b'x'; output_limit + 1]).unwrap();

    let exact = invoke(&root, &["cat", "exact"]);
    assert!(
        exact.status.success(),
        "{}",
        String::from_utf8_lossy(&exact.stderr)
    );
    assert_eq!(exact.stdout.len(), output_limit);

    let over = invoke(&root, &["cat", "over"]);
    assert_eq!(over.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&over.stderr).contains("output exceeds"));
    assert!(
        over.stdout.is_empty(),
        "single cat write must not partially exceed cap"
    );
}

#[test]
fn eleven_megabyte_file_is_denied_before_output() {
    let root = tempfile::tempdir().unwrap();
    let max_file = 10 * 1024 * 1024;
    fs::write(root.path().join("too_big"), vec![b'z'; max_file + 1]).unwrap();
    for args in [
        ["cat", "too_big"].as_slice(),
        ["head", "too_big"].as_slice(),
        ["tail", "too_big"].as_slice(),
        ["wc", "too_big"].as_slice(),
        ["grep", "hello", "too_big"].as_slice(),
    ] {
        let result = invoke(&root, args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("10 MiB"),
            "{args:?}"
        );
        assert!(result.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn bounded_line_counts_and_empty_outputs() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("small"), "a\nb\nc\n").unwrap();
    for (command, count) in [("head", "100001"), ("tail", "100001")] {
        let result = invoke(&root, &[command, "-n", count, "small"]);
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("too large"));
    }
    for command in ["head", "tail"] {
        let result = invoke(&root, &[command, "-n", "0", "small"]);
        assert!(result.status.success());
        assert!(result.stdout.is_empty());
    }
}

#[test]
#[ignore = "stress: creates 10001 files; run cargo test --test resource_limits -- --ignored"]
fn listing_above_ten_thousand_entries_is_denied() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..10_001 {
        fs::write(root.path().join(format!("item_{index:05}")), b"").unwrap();
    }
    let result = invoke(&root, &["ls"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("10000 entries"));
}


#[test]
#[ignore = "stress: creates 10001 hidden files; run cargo test --test resource_limits -- --ignored"]
fn hidden_entries_count_towards_the_directory_scan_limit() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..10_001 {
        fs::write(root.path().join(format!(".hidden_{index:05}")), b"").unwrap();
    }
    // Suppressing hidden names must not allow unbounded read_dir traversal.
    let result = invoke(&root, &["ls"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("10000 entries"));
}
