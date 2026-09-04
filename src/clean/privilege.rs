// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: EUID detection and privilege boundary management for voidctl clean

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Function pointer type for querying the effective user ID.
pub type EuidGetter = fn() -> u32;

/// Default production EUID query via libc.
#[must_use]
pub fn default_euid() -> u32 {
    // SAFETY: geteuid is a POSIX system call with no side effects and is thread-safe.
    unsafe { libc::geteuid() }
}

/// Returns whether the process is elevated using a supplied EUID provider.
#[must_use]
pub fn is_elevated_with(euid_fn: EuidGetter) -> bool {
    euid_fn() == 0
}

/// Returns whether the current process is executing with root privileges (EUID == 0).
#[must_use]
pub fn is_elevated() -> bool {
    is_elevated_with(default_euid)
}

/// Checks whether the current user has write permission to a path using POSIX access(W_OK).
#[must_use]
pub fn is_writable(path: &Path) -> bool {
    if is_elevated() {
        return true;
    }

    let check_path = if path.exists() {
        path.to_path_buf()
    } else if let Some(parent) = path.parent() {
        parent.to_path_buf()
    } else {
        return false;
    };

    if let Ok(c_str) = CString::new(check_path.as_os_str().as_bytes()) {
        // SAFETY: access() is standard POSIX checking real UID/GID against file permissions.
        unsafe { libc::access(c_str.as_ptr(), libc::W_OK) == 0 }
    } else {
        false
    }
}

/// Known system cache directories that can be safely read or scanned.
pub const SYSTEM_CACHE_ROOTS: &[&str] = &["/var/cache/pacman/pkg", "/var/log", "/tmp", "/var/tmp"];

/// Extended system roots scanned only when running with root privileges (EUID == 0).
pub const ELEVATED_SYSTEM_ROOTS: &[&str] = &["/var/cache", "/var/log/journal"];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn mock_root_euid() -> u32 {
        0
    }

    fn mock_user_euid() -> u32 {
        1000
    }

    #[test]
    fn test_is_elevated_seam() {
        assert!(is_elevated_with(mock_root_euid));
        assert!(!is_elevated_with(mock_user_euid));
    }

    #[test]
    fn test_is_elevated_callable() {
        // Must not panic in current environment
        let _ = is_elevated();
    }

    #[test]
    fn test_is_writable_on_tempdir() {
        let dir = tempdir().expect("tempdir");
        assert!(is_writable(dir.path()));
    }
}
