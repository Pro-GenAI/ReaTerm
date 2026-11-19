//! macOS process-level read-only enforcement via Apple's Seatbelt sandbox.
//!
//! This uses sandbox_init with an explicit custom profile, not any of the
//! deprecated pre-defined sandbox profile names. Apple's API is deprecated,
//! but this remains an available way to confine a standalone CLI on macOS.
//! Fail CLOSED if the macOS runtime rejects the profile. The Rust allowlist
//! and cap-std paths remain the primary command and path security checks.
//!
//! WARNING: Do not install Seatbelt in a normal unit-test process: a
//! successful sandbox is process-wide and permanent. Tests probe in a child.

use std::ffi::{c_char, c_int, CStr, CString};

#[link(name = "sandbox")]
extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

// Generic, no-write profile. A deny on file-write* covers data, create,
// unlink/rename, chmod/chown, xattrs and timestamp changes. Also deny
// hard-link creation, external process execution/forking and networking.
// No writable exceptions are installed, even for /tmp or ~/Library.
// Standard stdout/stderr are inherited from the launcher, not opened here.
//
// Keep the profile static: never concatenate user paths or variables into it.
// The caller-selected root is opened by cap-std before applying Seatbelt.
const READ_ONLY_PROFILE: &str = "(version 1)\n\
(allow default)\n\
(deny file-write*)\n\
(deny file-link)\n\
(deny process-exec)\n\
(deny process-fork)\n\
(deny network*)\n";

pub(crate) fn enforce_read_only() -> Result<(), String> {
    let profile = CString::new(READ_ONLY_PROFILE)
        .map_err(|_| "macOS sandbox profile contains a NUL byte".to_string())?;
    let mut error: *mut c_char = std::ptr::null_mut();
    // SAFETY: profile is an owned, NUL-terminated string for the entire call;
    // error is an out-pointer that sandbox_free_error releases when populated.
    let result = unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) };
    let details = if error.is_null() {
        None
    } else {
        // SAFETY: Apple guarantees the error buffer is NUL-terminated.
        let message = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: buffer is owned by libsandbox and must be freed by it.
        unsafe { sandbox_free_error(error) };
        Some(message)
    };
    if result != 0 {
        return Err(format!(
            "macOS Seatbelt sandbox enforcement failed (no unconfined fallback): {}",
            details.unwrap_or_else(|| std::io::Error::last_os_error().to_string())
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    const CHILD_KEY: &str = "REATERM_TEST_SEATBELT_CHILD";
    const TARGET_KEY: &str = "REATERM_TEST_SEATBELT_TARGET";

    #[test]
    fn seatbelt_blocks_direct_syscall_mutations_but_permits_reads() {
        // Spawn a single-test child because sandbox_init confines its process
        // permanently. The parent creates/cleans up the private fixture.
        if std::env::var_os(CHILD_KEY).is_some() {
            let target = std::path::PathBuf::from(std::env::var_os(TARGET_KEY).unwrap());
            let path = CString::new(target.as_os_str().as_bytes()).unwrap();
            let data = std::fs::read(&target).unwrap();
            assert_eq!(&data, b"unchanged\n");
            enforce_read_only().expect("macOS sandbox must install");
            assert_eq!(std::fs::read(&target).unwrap(), data);
            let mode_before = std::fs::metadata(&target).unwrap().permissions().mode();

            // SAFETY: the C path is valid for the duration of the call.
            // All operations are deliberately aimed at a disposable fixture.
            for (operation, call) in [
                ("chmod", unsafe { libc::chmod(path.as_ptr(), 0o777) }),
                ("chown", unsafe { libc::chown(path.as_ptr(), !0, !0) }),
            ] {
                assert_eq!(call, -1, "{operation} unexpectedly succeeded");
            }
            let other = target.with_extension("created");
            assert!(std::fs::write(&other, b"unsafe").is_err());
            assert!(!other.exists());
            assert!(std::fs::remove_file(&target).is_err());
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode(),
                mode_before
            );
            assert_eq!(std::fs::read(&target).unwrap(), b"unchanged\n");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("seatbelt-test");
        std::fs::write(&fixture, b"unchanged\n").unwrap();
        let mode_before = std::fs::metadata(&fixture).unwrap().permissions().mode();

        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "macos_guard::tests::seatbelt_blocks_direct_syscall_mutations_but_permits_reads",
                "--nocapture",
            ])
            .env(CHILD_KEY, "1")
            .env(TARGET_KEY, &fixture)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "Seatbelt child failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert_eq!(std::fs::read(&fixture).unwrap(), b"unchanged\n");
        assert_eq!(
            std::fs::metadata(&fixture).unwrap().permissions().mode(),
            mode_before
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
