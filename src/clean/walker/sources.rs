// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Specialized batch cleanup target scanners for pacman cache, trash, user cache, dotfiles backups, and journal

use super::size::{calculate_dir_size, count_and_size_dir, parse_journal_disk_usage};
use crate::clean::classifier::logs_cache::is_older_than;
use crate::clean::classifier::{CleanCategory, CleanTarget};
use crate::clean::privilege::is_writable;
use std::fs;
use std::path::Path;

/// Formats a readable title for a build target directory.
#[must_use]
pub fn format_build_target_title(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("target");
    let project = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("project");

    match name {
        "target" => format!("Cargo Target ({project})"),
        "node_modules" => format!("node_modules ({project})"),
        "__pycache__" => format!("Python cache ({project})"),
        _ => format!("Build Artifact ({project}/{name})"),
    }
}

/// Scans /var/cache/pacman/pkg for cached packages and signatures.
#[must_use]
pub fn scan_pacman_cache(pacman_dir: &Path) -> Option<CleanTarget> {
    if !pacman_dir.exists() {
        return None;
    }

    let entries = fs::read_dir(pacman_dir).ok()?;
    let mut files = Vec::new();
    let mut total_size: u64 = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };

        let is_pkg = name.ends_with(".pkg.tar.zst")
            || name.ends_with(".pkg.tar.xz")
            || name.ends_with(".sig")
            || name.ends_with(".part");

        if is_pkg && let Ok(meta) = entry.metadata() {
            total_size += meta.len();
            files.push(path);
        }
    }

    if files.is_empty() {
        return None;
    }

    let count = files.len();
    Some(CleanTarget::new(
        "Pacman Package Cache".to_string(),
        pacman_dir.to_path_buf(),
        CleanCategory::PackageCache,
        total_size,
        count,
        !is_writable(pacman_dir),
        false,
        files,
        format!("{count} cached packages & signatures"),
    ))
}

/// Scans user Trash directory.
#[must_use]
pub fn scan_trash(trash_dir: &Path) -> Option<CleanTarget> {
    if !trash_dir.exists() {
        return None;
    }

    let files_dir = trash_dir.join("files");
    let info_dir = trash_dir.join("info");
    let mut files = Vec::new();
    let mut total_size: u64 = 0;

    for dir in [&files_dir, &info_dir] {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        total_size += calculate_dir_size(&p);
                    } else {
                        total_size += meta.len();
                    }
                    files.push(p);
                }
            }
        }
    }

    if files.is_empty() {
        return None;
    }

    let count = files.len();
    Some(CleanTarget::new(
        "User Trash Bin".to_string(),
        trash_dir.to_path_buf(),
        CleanCategory::Trash,
        total_size,
        count,
        !is_writable(trash_dir),
        false,
        files,
        format!("{count} files in Trash"),
    ))
}

/// Scans direct subdirectories within ~/.cache and aggregates application caches.
#[must_use]
pub fn scan_user_cache_root(cache_root: &Path, age_threshold: u64) -> Vec<CleanTarget> {
    let mut targets = Vec::new();
    let entries = match fs::read_dir(cache_root) {
        Ok(e) => e,
        Err(_) => return targets,
    };

    for entry in entries.flatten() {
        let p = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !meta.is_dir() {
            continue;
        }

        let name = match p.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };

        let (size, count) = count_and_size_dir(&p);
        if count == 0 || size < 1024 {
            continue;
        }

        let requires_elevation = !is_writable(&p);

        match name {
            "thumbnails" => {
                targets.push(CleanTarget::new(
                    "User Thumbnail Cache".to_string(),
                    p,
                    CleanCategory::Thumbnails,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} cached image thumbnails"),
                ));
            }
            "yay" => {
                targets.push(CleanTarget::new(
                    "Yay AUR Build Cache".to_string(),
                    p,
                    CleanCategory::PackageCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} build artifacts & packages"),
                ));
            }
            "paru" => {
                targets.push(CleanTarget::new(
                    "Paru AUR Build Cache".to_string(),
                    p,
                    CleanCategory::PackageCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} build artifacts & packages"),
                ));
            }
            "uv" => {
                targets.push(CleanTarget::new(
                    "uv Python Cache".to_string(),
                    p,
                    CleanCategory::PackageCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} cached Python packages & wheels"),
                ));
            }
            "pip" => {
                targets.push(CleanTarget::new(
                    "pip Cache".to_string(),
                    p,
                    CleanCategory::PackageCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} cached pip wheels"),
                ));
            }
            "ccache" => {
                targets.push(CleanTarget::new(
                    "ccache Compiler Cache".to_string(),
                    p,
                    CleanCategory::LogsCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} compiled objects"),
                ));
            }
            "google-chrome" => {
                targets.push(CleanTarget::new(
                    "Google Chrome Cache".to_string(),
                    p,
                    CleanCategory::LogsCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} browser cache files"),
                ));
            }
            "BraveSoftware" => {
                targets.push(CleanTarget::new(
                    "Brave Browser Cache".to_string(),
                    p,
                    CleanCategory::LogsCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} browser cache files"),
                ));
            }
            "go-build" => {
                targets.push(CleanTarget::new(
                    "Go Build Cache".to_string(),
                    p,
                    CleanCategory::LogsCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} cached Go build targets"),
                ));
            }
            "flatpak" => {
                targets.push(CleanTarget::new(
                    "Flatpak Cache".to_string(),
                    p,
                    CleanCategory::LogsCache,
                    size,
                    count,
                    requires_elevation,
                    true,
                    Vec::new(),
                    format!("{count} flatpak cache entries"),
                ));
            }
            _ => {
                if is_older_than(&meta, age_threshold) && size >= 10 * 1024 * 1024 {
                    targets.push(CleanTarget::new(
                        format!("App Cache ({name})"),
                        p,
                        CleanCategory::LogsCache,
                        size,
                        count,
                        requires_elevation,
                        true,
                        Vec::new(),
                        format!("Cache older than {age_threshold} days"),
                    ));
                }
            }
        }
    }

    targets
}

/// Scans stale dotfiles backup snapshot subfolders.
#[must_use]
pub fn scan_dotfiles_backups(backup_dir: &Path, age_threshold: u64) -> Vec<CleanTarget> {
    let mut targets = Vec::new();
    if !backup_dir.exists() {
        return targets;
    }

    if let Ok(entries) = fs::read_dir(backup_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = entry.metadata()
                && meta.is_dir()
                && is_older_than(&meta, age_threshold)
            {
                let (size, count) = count_and_size_dir(&p);
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("backup");
                targets.push(CleanTarget::new(
                    format!("Stale Dotfiles Backup ({name})"),
                    p.clone(),
                    CleanCategory::Backups,
                    size,
                    count,
                    !is_writable(&p),
                    true,
                    Vec::new(),
                    format!("Backup snapshot older than {age_threshold} days"),
                ));
            }
        }
    }

    targets
}

/// Scans systemd journal logs using /var/log/journal and journalctl --disk-usage.
#[must_use]
pub fn scan_journal_logs() -> Option<CleanTarget> {
    let journal_dir = Path::new("/var/log/journal");
    if journal_dir.exists() {
        let (size, count) = count_and_size_dir(journal_dir);
        if size > 0 {
            return Some(CleanTarget::new(
                "Systemd Journal Logs".to_string(),
                journal_dir.to_path_buf(),
                CleanCategory::LogsCache,
                size,
                count.max(1),
                !is_writable(journal_dir),
                false,
                Vec::new(),
                "Archived journals (vacuum with: sudo journalctl --vacuum-time=14d)".to_string(),
            ));
        }
    }

    if let Ok(output) = std::process::Command::new("journalctl")
        .arg("--disk-usage")
        .output()
        && output.status.success()
    {
        let out_str = String::from_utf8_lossy(&output.stdout);
        if let Some(bytes) = parse_journal_disk_usage(&out_str)
            && bytes > 0
        {
            return Some(CleanTarget::new(
                "Systemd Journal Logs".to_string(),
                journal_dir.to_path_buf(),
                CleanCategory::LogsCache,
                bytes,
                1,
                true,
                false,
                Vec::new(),
                "Archived journals (vacuum with: sudo journalctl --vacuum-time=14d)".to_string(),
            ));
        }
    }

    None
}
