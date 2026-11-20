//! Minimal Linux seccomp complement to Landlock.
//!
//! Landlock ABI v3 controls file writes, creation, removal, and truncation,
//! but not chmod/chown/xattr/timestamp operations. Also deny exec, clone,
//! socket, io_uring setup, and direct truncation syscalls.
//! This is defense in depth: ReaTerm still only dispatches native read commands.
//!
//! The policy is deliberately fail-closed on unsupported architectures.

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(crate) fn enforce_metadata_read_only() -> Result<(), String> {
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    #[cfg(target_arch = "x86_64")]
    const BPF_JMP_JSET_K: u16 = 0x45;
    const BPF_RET_K: u16 = 0x06;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
    const SECCOMP_RET_ERRNO_EPERM: u32 = 0x0005_0000 | (libc::EPERM as u32);

    #[cfg(target_arch = "x86_64")]
    const ARCH: u32 = 0xc000_003e; // AUDIT_ARCH_X86_64
    #[cfg(target_arch = "aarch64")]
    const ARCH: u32 = 0xc000_00b7; // AUDIT_ARCH_AARCH64

    // Only native syscall ABIs are accepted. The x32 high syscall bit
    // must not bypass any filter comparisons on x86_64.
    #[cfg(target_arch = "x86_64")]
    let denied: &[libc::c_long] = &[
        libc::SYS_chmod,
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        libc::SYS_fchmodat2,
        libc::SYS_chown,
        libc::SYS_fchown,
        libc::SYS_lchown,
        libc::SYS_fchownat,
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        libc::SYS_utime,
        libc::SYS_utimes,
        libc::SYS_futimesat,
        libc::SYS_utimensat,
        libc::SYS_mknod,
        libc::SYS_mknodat,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_open_tree,
        libc::SYS_move_mount,
        libc::SYS_fsopen,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_mount_setattr,
        // Process execution, new threads/processes, outbound networking,
        // async I/O rings and descriptor-based truncation are prohibited.
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_clone,
        435, // clone3 on both x86_64 and arm64
        libc::SYS_socket,
        libc::SYS_connect,
        425, // io_uring_setup
        libc::SYS_ftruncate,
    ];
    #[cfg(target_arch = "aarch64")]
    let denied: &[libc::c_long] = &[
        libc::SYS_fchmod,
        libc::SYS_fchmodat,
        452, // __NR_fchmodat2 in Linux asm-generic/unistd.h (libc may omit this on ARM64)
        libc::SYS_fchown,
        libc::SYS_fchownat,
        libc::SYS_setxattr,
        libc::SYS_lsetxattr,
        libc::SYS_fsetxattr,
        libc::SYS_removexattr,
        libc::SYS_lremovexattr,
        libc::SYS_fremovexattr,
        libc::SYS_utimensat,
        libc::SYS_mknodat,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_open_tree,
        libc::SYS_move_mount,
        libc::SYS_fsopen,
        libc::SYS_fsconfig,
        libc::SYS_fsmount,
        libc::SYS_mount_setattr,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_clone,
        435, // clone3 on both x86_64 and arm64
        libc::SYS_socket,
        libc::SYS_connect,
        425, // io_uring_setup
        libc::SYS_ftruncate,
    ];
    let ins = |code: u16, jt: u8, jf: u8, k: u32| libc::sock_filter { code, jt, jf, k };
    let mut filter = Vec::with_capacity(7 + 2 * denied.len());

    // seccomp_data.arch lives at offset 4; reject unexpected ABIs.
    filter.push(ins(BPF_LD_W_ABS, 0, 0, 4));
    filter.push(ins(BPF_JMP_JEQ_K, 1, 0, ARCH));
    filter.push(ins(BPF_RET_K, 0, 0, SECCOMP_RET_KILL_PROCESS));

    // seccomp_data.nr lives at offset 0.
    filter.push(ins(BPF_LD_W_ABS, 0, 0, 0));
    #[cfg(target_arch = "x86_64")]
    {
        filter.push(ins(BPF_JMP_JSET_K, 0, 1, 0x4000_0000));
        filter.push(ins(BPF_RET_K, 0, 0, SECCOMP_RET_ERRNO_EPERM));
    }
    for &syscall in denied {
        filter.push(ins(BPF_JMP_JEQ_K, 0, 1, syscall as u32));
        filter.push(ins(BPF_RET_K, 0, 0, SECCOMP_RET_ERRNO_EPERM));
    }
    filter.push(ins(BPF_RET_K, 0, 0, SECCOMP_RET_ALLOW));

    let mut prog = libc::sock_fprog {
        len: u16::try_from(filter.len()).map_err(|_| "seccomp policy is too large")?,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: The BPF program points at a valid in-memory array of
    // classic socket-filter instructions, which remains alive for prctl.
    // prctl copies the filter before returning. We check every return value.
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
            return Err(format!(
                "cannot set no_new_privs: {}",
                std::io::Error::last_os_error()
            ));
        }
        if libc::prctl(libc::PR_SET_SECCOMP, libc::SECCOMP_MODE_FILTER, &mut prog) != 0 {
            return Err(format!(
                "cannot install read-only metadata seccomp filter: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
pub(crate) fn enforce_metadata_read_only() -> Result<(), String> {
    Err("metadata protection requires Linux x86_64 or aarch64 with seccomp".into())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn seccomp_denies_chmod() {
        let fixture = tempfile::tempdir().expect("disposable fixture root");
        let path = fixture.path().join("permissions");
        std::fs::write(&path, b"unchanged").unwrap();
        // Run the restrictive filter on a dedicated test thread, not the
        // global test runner. Only attempt a chmod on disposable test data.
        std::thread::spawn(move || {
            enforce_metadata_read_only().expect("seccomp must be enforced");
            let original_mode = std::fs::metadata(&path).unwrap().permissions().mode();
            let permissions = std::fs::Permissions::from_mode(original_mode);
            let denied = std::fs::set_permissions(&path, permissions).unwrap_err();
            assert_eq!(denied.raw_os_error(), Some(libc::EPERM));
        })
        .join()
        .expect("seccomp test thread");
    }

    #[test]
    fn seccomp_denies_process_network_and_truncation_syscalls() {
        use std::os::fd::AsRawFd;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("readable");
        std::fs::write(&path, b"unchanged").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        std::thread::spawn(move || {
            enforce_metadata_read_only().expect("seccomp policy required");
            let denied = |name: &str, result: libc::c_long| {
                assert_eq!(result, -1, "{name} unexpectedly succeeded");
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EPERM),
                    "{name} must be blocked before kernel dispatch"
                );
            };
            // SAFETY: syscall arguments are inert and the seccomp filter
            // refuses them before dispatch. Do not execute a child process.
            unsafe {
                denied("fork", libc::syscall(libc::SYS_fork));
                denied("clone3", libc::syscall(435, std::ptr::null::<u8>(), 0));
                denied(
                    "socket",
                    libc::syscall(libc::SYS_socket, libc::AF_INET, libc::SOCK_STREAM, 0),
                );
                denied(
                    "io_uring_setup",
                    libc::syscall(425, 1, std::ptr::null_mut::<u8>()),
                );
                denied(
                    "ftruncate",
                    libc::syscall(libc::SYS_ftruncate, file.as_raw_fd(), 0),
                );
            }
        })
        .join()
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"unchanged");
    }

    #[test]
    fn seccomp_blocks_metadata_mutations_on_disposable_file() {
        use std::ffi::CString;
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;

        let root = tempfile::tempdir().expect("private temporary fixture");
        let path = root.path().join("metadata_fixture");
        std::fs::write(&path, b"unchanged\n").unwrap();
        let before = std::fs::metadata(&path).unwrap();
        let mode = before.permissions().mode();
        let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let name = CString::new("user.reaterm_test").unwrap();
        let value = b"changed";
        let fd = std::fs::File::open(&path).unwrap();

        std::thread::spawn(move || {
            enforce_metadata_read_only().expect("seccomp required");
            // SAFETY: all pointers and file descriptors are valid for these
            // syscalls, which are expected to be rejected by the filter.
            // SAFETY: all pointers and file descriptors are valid. Each
            // syscall should be rejected immediately by seccomp with EPERM.
            let denied = |name: &str, result: libc::c_int| {
                assert_eq!(result, -1, "{name} unexpectedly succeeded");
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::EPERM),
                    "{name} must be denied by the seccomp filter"
                );
            };
            unsafe {
                denied("chmod", libc::chmod(c_path.as_ptr(), 0o777));
                denied("fchmod", libc::fchmod(fd.as_raw_fd(), 0o777));
                denied("chown", libc::chown(c_path.as_ptr(), !0, !0));
                denied("fchown", libc::fchown(fd.as_raw_fd(), !0, !0));
                denied(
                    "setxattr",
                    libc::setxattr(
                        c_path.as_ptr(),
                        name.as_ptr(),
                        value.as_ptr().cast(),
                        value.len(),
                        0,
                    ),
                );
                denied(
                    "utimensat",
                    libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), std::ptr::null(), 0),
                );
            }
        })
        .join()
        .expect("seccomp worker thread");

        let after = std::fs::metadata(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"unchanged\n");
        assert_eq!(after.permissions().mode(), mode);
        assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    }
}
