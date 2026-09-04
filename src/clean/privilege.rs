// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: EUID detection and privilege boundary management for voidctl clean

/// Returns whether the current process is executing with root privileges (EUID == 0).
#[must_use]
pub fn is_elevated() -> bool {
    // SAFETY: geteuid is a POSIX system call with no side effects and is thread-safe.
    unsafe { libc::geteuid() == 0 }
}

/// Known system cache directories that can be safely read or scanned.
pub const SYSTEM_CACHE_ROOTS: &[&str] = &["/var/cache/pacman/pkg", "/var/log", "/tmp", "/var/tmp"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_elevated_callable() {
        // Must not panic
        let _ = is_elevated();
    }
}
