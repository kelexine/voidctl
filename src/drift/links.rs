// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Dotfiles symlink integrity and mode-bit permission drift verification

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Status classification of a configured symlink pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    /// Symlink exists, points to source, and permission mode bits match.
    Valid,
    /// Symlink points to source, but POSIX permission bits differ from git/security rules.
    PermissionDrift {
        expected_mode: u32,
        actual_mode: u32,
    },
    /// Symlink exists but points to an incorrect destination.
    Broken { actual_destination: PathBuf },
    /// Target path is an ordinary file or directory instead of a symlink.
    ReplacedByRealFile,
    /// Target path does not exist at all.
    Missing,
    /// Source file does not exist in the dotfiles repository.
    MissingSource,
}

impl fmt::Display for LinkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "VALID"),
            Self::PermissionDrift {
                expected_mode,
                actual_mode,
            } => {
                write!(f, "MODE DRIFT ({:o} vs {:o})", expected_mode, actual_mode)
            }
            Self::Broken { actual_destination } => {
                write!(f, "BROKEN (points to {})", actual_destination.display())
            }
            Self::ReplacedByRealFile => write!(f, "REPLACED BY FILE"),
            Self::Missing => write!(f, "MISSING LINK"),
            Self::MissingSource => write!(f, "SOURCE MISSING IN REPO"),
        }
    }
}

/// Verification record for a single mapped symlink pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRecord {
    pub source_rel: String,
    pub target_rel: String,
    pub expected_source: PathBuf,
    pub target_path: PathBuf,
    pub status: LinkStatus,
}

/// Verifies all configured symlink mappings against dotfiles_dir and home_dir.
#[must_use]
pub fn verify_symlinks(
    dotfiles_dir: &Path,
    home_dir: &Path,
    links: &HashMap<String, String>,
) -> Vec<LinkRecord> {
    let mut records = Vec::with_capacity(links.len());

    for (source_rel, target_rel) in links {
        let expected_source = dotfiles_dir.join(source_rel);
        let target_path = home_dir.join(target_rel);
        let status = evaluate_link(dotfiles_dir, source_rel, &expected_source, &target_path);

        records.push(LinkRecord {
            source_rel: source_rel.clone(),
            target_rel: target_rel.clone(),
            expected_source,
            target_path,
            status,
        });
    }

    records.sort_by(|a, b| a.target_rel.cmp(&b.target_rel));
    records
}

/// Evaluates a single symlink target against its expected repository source.
#[must_use]
pub fn evaluate_link(
    dotfiles_dir: &Path,
    source_rel: &str,
    expected_source: &Path,
    target_path: &Path,
) -> LinkStatus {
    if !expected_source.exists() {
        return LinkStatus::MissingSource;
    }

    let symlink_meta = match fs::symlink_metadata(target_path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return LinkStatus::Missing,
        Err(_) => return LinkStatus::Missing,
    };

    if !symlink_meta.file_type().is_symlink() {
        return LinkStatus::ReplacedByRealFile;
    }

    let resolved_dest = match fs::read_link(target_path) {
        Ok(dest) => dest,
        Err(_) => {
            return LinkStatus::Broken {
                actual_destination: PathBuf::from("<unreadable>"),
            };
        }
    };

    let full_dest = if resolved_dest.is_relative() {
        if let Some(parent) = target_path.parent() {
            parent.join(&resolved_dest)
        } else {
            resolved_dest.clone()
        }
    } else {
        resolved_dest.clone()
    };

    let canonical_dest = full_dest.canonicalize().unwrap_or(full_dest);
    let canonical_src = expected_source
        .canonicalize()
        .unwrap_or_else(|_| expected_source.to_path_buf());

    if canonical_dest != canonical_src {
        return LinkStatus::Broken {
            actual_destination: resolved_dest,
        };
    }

    check_mode_drift(dotfiles_dir, source_rel, expected_source)
}

/// Checks mode drift against git tracked mode and security constraints.
#[must_use]
pub fn check_mode_drift(dotfiles_dir: &Path, source_rel: &str, source: &Path) -> LinkStatus {
    let disk_mode = fs::metadata(source)
        .map(|m| m.permissions().mode() & 0o777)
        .unwrap_or(0);

    // Check SSH permissions: ssh configs must not be world or group writable
    if source_rel.ends_with("ssh/config") && (disk_mode & 0o022) != 0 {
        return LinkStatus::PermissionDrift {
            expected_mode: 0o600,
            actual_mode: disk_mode,
        };
    }

    // Check against git index if dotfiles_dir is a git repository
    if let Some(git_mode) = query_git_mode(dotfiles_dir, source_rel) {
        let git_perm = git_mode & 0o777;
        let exec_drift = (git_perm & 0o111) != (disk_mode & 0o111);
        if exec_drift {
            return LinkStatus::PermissionDrift {
                expected_mode: git_perm,
                actual_mode: disk_mode,
            };
        }
    }

    LinkStatus::Valid
}

/// Queries git index mode for the specified relative path.
fn query_git_mode(dotfiles_dir: &Path, source_rel: &str) -> Option<u32> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dotfiles_dir)
        .arg("ls-files")
        .arg("--stage")
        .arg(source_rel)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next()?;
    let mode_str = first_line.split_whitespace().next()?;
    u32::from_str_radix(mode_str, 8).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn test_verify_symlink_valid() {
        let dir = tempdir().expect("tempdir");
        let dotfiles = dir.path().join("dotfiles");
        let home = dir.path().join("home");
        fs::create_dir_all(&dotfiles).expect("create dotfiles");
        fs::create_dir_all(&home).expect("create home");

        let src_file = dotfiles.join("testrc");
        fs::write(&src_file, "alias test=1").expect("write src");

        let target_file = home.join(".testrc");
        symlink(&src_file, &target_file).expect("create symlink");

        let mut map = HashMap::new();
        map.insert("testrc".to_string(), ".testrc".to_string());

        let results = verify_symlinks(&dotfiles, &home, &map);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, LinkStatus::Valid);
    }
}
