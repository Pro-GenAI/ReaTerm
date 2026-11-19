//! Windows read-only enforcement using a write-restricted access token.
//!
//! The native commands never spawn processes, invoke a shell, or expose
//! mutation APIs. This kernel access check is an additional barrier to
//! accidental filesystem writes, ACL changes, and deletions.
//!
//! A write-restricted token and a mandatory low-integrity label restrict
//! writes and deletion against normally protected objects. The extra
//! restricting SID is freshly randomized and has no explicit write grants
//! on ordinary file DACLs. Reads continue to use the original identity.
//! The token is impersonated by the current command thread BEFORE opening
//! the capability directory. Failure to initialize it aborts the command.
//!
//! This is a THREAD impersonation barrier (not a process-wide AppContainer).
//! Never add worker threads, external process execution, or inherited writable
//! handles to the command dispatcher without a separate security review.
//! An object with a null DACL is outside the restricting-SID guarantee.

use std::ffi::c_void;
use std::ptr;

type Handle = *mut c_void;

#[repr(C)]
struct SidIdentifierAuthority {
    value: [u8; 6],
}

#[repr(C)]
struct SidAndAttributes {
    sid: *mut c_void,
    attributes: u32,
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: Handle, desired_access: u32, token: *mut Handle) -> i32;
    fn CreateRestrictedToken(
        existing: Handle,
        flags: u32,
        disable_count: u32,
        sids_to_disable: *mut SidAndAttributes,
        privilege_count: u32,
        privileges_to_delete: *mut c_void,
        restrict_count: u32,
        restricting_sids: *mut SidAndAttributes,
        new_token: *mut Handle,
    ) -> i32;
    fn AllocateAndInitializeSid(
        authority: *const SidIdentifierAuthority,
        count: u8,
        one: u32,
        two: u32,
        three: u32,
        four: u32,
        five: u32,
        six: u32,
        seven: u32,
        eight: u32,
        output: *mut *mut c_void,
    ) -> i32;
    fn FreeSid(sid: *mut c_void) -> *mut c_void;
    fn IsTokenRestricted(token: Handle) -> i32;
    fn ImpersonateLoggedOnUser(token: Handle) -> i32;
    fn SetTokenInformation(
        token: Handle,
        token_information_class: i32,
        token_information: *const c_void,
        token_information_length: u32,
    ) -> i32;
    fn GetLengthSid(sid: *const c_void) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
}

#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, len: u32, flags: u32) -> i32;
}

struct OwnedHandle(Handle);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle belongs to this instance and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

struct OwnedSid(*mut c_void);

impl Drop for OwnedSid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: SID was allocated by AllocateAndInitializeSid.
            unsafe { FreeSid(self.0) };
        }
    }
}

fn win_error(operation: &str) -> String {
    format!(
        "Windows read-only token {operation} failed (no unconfined fallback): {}",
        std::io::Error::last_os_error()
    )
}

/// Establish kernel-level write restrictions on the thread that executes the
/// native read-only command. Do not spawn other threads after this call.
pub(crate) fn enforce_read_only() -> Result<(), String> {
    const TOKEN_DUPLICATE: u32 = 0x0002;
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ADJUST_DEFAULT: u32 = 0x0080;
    const TOKEN_INTEGRITY_LEVEL: i32 = 25;
    const SE_GROUP_INTEGRITY: u32 = 0x0000_0020;
    const SECURITY_MANDATORY_LOW_RID: u32 = 0x0000_1000;
    const DISABLE_MAX_PRIVILEGE: u32 = 0x0001;
    const WRITE_RESTRICTED: u32 = 0x0008;
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;

    // A new, unguessable SID for every invocation. This SID does not have
    // write ACEs in existing normal DACLs, including the caller's temp folder.
    // SAFETY: BCryptGenRandom fills the 16-byte writable stack buffer.
    let mut nonce = [0u8; 16];
    let status = unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            nonce.as_mut_ptr(),
            nonce.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err(format!(
            "Windows secure random SID generation failed (NTSTATUS {status:#x})"
        ));
    }
    let words: [u32; 4] = std::array::from_fn(|i| {
        u32::from_le_bytes(nonce[(i * 4)..(i * 4 + 4)].try_into().unwrap())
    });
    let authority = SidIdentifierAuthority {
        value: [0, 0, 0, 0, 0, 5],
    };
    let mut sid_ptr: *mut c_void = ptr::null_mut();
    // SAFETY: authority and out-pointer are valid; there are four subauthorities.
    if unsafe {
        AllocateAndInitializeSid(
            &authority,
            4,
            words[0],
            words[1],
            words[2],
            words[3],
            0,
            0,
            0,
            0,
            &mut sid_ptr,
        )
    } == 0
    {
        return Err(win_error("AllocateAndInitializeSid"));
    }
    let sid = OwnedSid(sid_ptr);

    let mut process_token: Handle = ptr::null_mut();
    // SAFETY: the process pseudo-handle and output pointer are valid.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT,
            &mut process_token,
        )
    } == 0
    {
        return Err(win_error("OpenProcessToken"));
    }
    let process_token = OwnedHandle(process_token);

    let mut restriction = SidAndAttributes {
        sid: sid.0,
        attributes: 0,
    };
    let mut limited_token: Handle = ptr::null_mut();
    // SAFETY: source token and SID are valid for the call; there are no
    // disabled group or privilege arrays. All nonessential privileges are
    // dropped, and writes require this freshly generated restricting SID.
    if unsafe {
        CreateRestrictedToken(
            process_token.0,
            DISABLE_MAX_PRIVILEGE | WRITE_RESTRICTED,
            0,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            1,
            &mut restriction,
            &mut limited_token,
        )
    } == 0
    {
        return Err(win_error("CreateRestrictedToken"));
    }
    let limited_token = OwnedHandle(limited_token);

    // Windows WRITE_RESTRICTED prevents ordinary file-data writes, but
    // DELETE may not count as a "write" in that restricted-SID check.
    // Mandatory Integrity Control NO_WRITE_UP also denies DELETE and ACL
    // changes against ordinary medium/high integrity filesystem objects.
    // It does not control low-integrity objects or null-DACL objects.
    // The command allowlist remains the primary mutation barrier.
    let low_authority = SidIdentifierAuthority {
        value: [0, 0, 0, 0, 0, 16],
    };
    let mut low_sid_ptr: *mut c_void = ptr::null_mut();
    // SAFETY: one-subauthority low-integrity SID is initialized by Windows.
    if unsafe {
        AllocateAndInitializeSid(
            &low_authority,
            1,
            SECURITY_MANDATORY_LOW_RID,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut low_sid_ptr,
        )
    } == 0
    {
        return Err(win_error("AllocateAndInitializeSid(low integrity)"));
    }
    let low_sid = OwnedSid(low_sid_ptr);
    let label = SidAndAttributes {
        sid: low_sid.0,
        attributes: SE_GROUP_INTEGRITY,
    };
    // TOKEN_MANDATORY_LABEL contains exactly one SID_AND_ATTRIBUTES.
    // SAFETY: label and its SID are live; the token allows adjustment of
    // its default integrity level. The new level is strictly lower.
    let label_size =
        std::mem::size_of::<SidAndAttributes>() as u32 + unsafe { GetLengthSid(low_sid.0) };
    if unsafe {
        SetTokenInformation(
            limited_token.0,
            TOKEN_INTEGRITY_LEVEL,
            (&label as *const SidAndAttributes).cast(),
            label_size,
        )
    } == 0
    {
        return Err(win_error("SetTokenInformation(low integrity)"));
    }

    // An accidentally empty restricting-SID set must not disable the filter.
    // SAFETY: limited_token is a live access token.
    if unsafe { IsTokenRestricted(limited_token.0) } == 0 {
        return Err("Windows write-restricted token is missing its restricting SID".into());
    }

    // This operation is persistent on the current command thread. In
    // particular, do not call RevertToSelf after installing the token.
    // SAFETY: the kernel duplicates the token reference for impersonation.
    if unsafe { ImpersonateLoggedOnUser(limited_token.0) } == 0 {
        return Err(win_error("ImpersonateLoggedOnUser"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn restricted_token_denies_direct_file_mutations() {
        let dir = tempfile::tempdir().expect("disposable fixture root");
        let file = dir.path().join("readable.txt");
        let created = dir.path().join("unauthorized.txt");
        let renamed = dir.path().join("renamed.txt");
        fs::write(&file, b"unchanged\n").unwrap();
        let contents = fs::read(&file).unwrap();
        let before = fs::metadata(&file).unwrap();

        // Impersonation affects this thread, not the rest of the test runner.
        // Only private fixtures are targeted.
        let worker = std::thread::spawn(move || {
            enforce_read_only().expect("Windows kernel restrictions required");
            assert_eq!(fs::read(&file).unwrap(), b"unchanged\n");
            assert!(fs::OpenOptions::new().write(true).open(&file).is_err());
            assert!(fs::write(&file, b"modified").is_err());
            assert!(fs::write(&created, b"created").is_err());
            assert!(fs::remove_file(&file).is_err());
            assert!(fs::rename(&file, &renamed).is_err());
            let mut permissions = fs::metadata(&file).unwrap().permissions();
            permissions.set_readonly(true);
            assert!(fs::set_permissions(&file, permissions).is_err());
            assert_eq!(fs::read(&file).unwrap(), b"unchanged\n");

            // No subprocess may be introduced in the actual command set.
            // A write-restricted *thread* is not a process-wide restriction;
            // therefore we do not make an unenforceable claim about children.
        });
        worker.join().expect("restricted worker thread");

        assert_eq!(fs::read(dir.path().join("readable.txt")).unwrap(), contents);
        assert_eq!(
            fs::metadata(dir.path().join("readable.txt")).unwrap().len(),
            before.len()
        );
        assert!(!dir.path().join("unauthorized.txt").exists());
        assert!(!dir.path().join("renamed.txt").exists());
    }
}
