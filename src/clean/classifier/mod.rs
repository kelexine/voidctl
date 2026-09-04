// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Classification models and category definitions for system hygiene

pub mod artifacts;
pub mod hotspots;
pub mod logs_cache;

use std::fmt;
use std::path::PathBuf;

/// Cleanup category classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CleanCategory {
    /// Pacman / AUR package caches.
    PackageCache,
    /// Build artifacts (e.g. target/, node_modules/, __pycache__/).
    Artifacts,
    /// User thumbnail caches.
    Thumbnails,
    /// Deleted files in user trash bin.
    Trash,
    /// Historical dotfiles backups.
    Backups,
    /// Aged log files and cache hierarchies.
    LogsCache,
    /// High-capacity storage consumers.
    Hotspots,
}

impl fmt::Display for CleanCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageCache => write!(f, "Package Cache"),
            Self::Artifacts => write!(f, "Build Artifacts"),
            Self::Thumbnails => write!(f, "Thumbnails"),
            Self::Trash => write!(f, "User Trash"),
            Self::Backups => write!(f, "Dotfiles Backups"),
            Self::LogsCache => write!(f, "Logs & Cache"),
            Self::Hotspots => write!(f, "Disk Hotspots"),
        }
    }
}

impl std::str::FromStr for CleanCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase().replace(['-', '_', ' ', '&'], "");
        match normalized.as_str() {
            "packagecache" | "package" | "packages" | "pkg" => Ok(Self::PackageCache),
            "artifacts" | "artifact" | "build" | "buildartifacts" => Ok(Self::Artifacts),
            "thumbnails" | "thumbnail" | "thumbs" => Ok(Self::Thumbnails),
            "trash" | "trashbin" | "usertrash" => Ok(Self::Trash),
            "backups" | "backup" | "dotfilesbackups" => Ok(Self::Backups),
            "logscache" | "logs" | "cache" | "log" | "logsandcache" => Ok(Self::LogsCache),
            "hotspots" | "hotspot" | "diskhotspots" => Ok(Self::Hotspots),
            _ => Err(format!("Unknown cleanup category: '{s}'")),
        }
    }
}

impl CleanCategory {
    /// Returns all known categories.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::PackageCache,
            Self::LogsCache,
            Self::Thumbnails,
            Self::Artifacts,
            Self::Trash,
            Self::Backups,
            Self::Hotspots,
        ]
    }
}

/// An aggregated cleanup target presented to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanTarget {
    /// Human-friendly display label (e.g. "Pacman Package Cache", "User Thumbnail Cache").
    pub title: String,
    /// Canonical directory or file path.
    pub path: PathBuf,
    /// Category classification.
    pub category: CleanCategory,
    /// Total reclaimable size in bytes.
    pub size_bytes: u64,
    /// Number of items contained (e.g. 1308 packages, or 1 tree).
    pub item_count: usize,
    /// Whether elevated privileges (root) are required to delete this target.
    pub requires_elevation: bool,
    /// Whether deleting this target removes an entire directory tree.
    pub is_tree: bool,
    /// Explicit list of files if target is a batch of files rather than a whole tree.
    pub files: Vec<PathBuf>,
    /// Informative reason/explanation.
    pub reason: String,
}

impl CleanTarget {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: String,
        path: PathBuf,
        category: CleanCategory,
        size_bytes: u64,
        item_count: usize,
        requires_elevation: bool,
        is_tree: bool,
        files: Vec<PathBuf>,
        reason: String,
    ) -> Self {
        Self {
            title,
            path,
            category,
            size_bytes,
            item_count,
            requires_elevation,
            is_tree,
            files,
            reason,
        }
    }
}

/// A classified cleanup candidate item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub category: CleanCategory,
    pub reason: String,
    pub is_dir: bool,
}

impl CleanItem {
    #[must_use]
    pub fn new(
        path: PathBuf,
        size_bytes: u64,
        category: CleanCategory,
        reason: String,
        is_dir: bool,
    ) -> Self {
        Self {
            path,
            size_bytes,
            category,
            reason,
            is_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_clean_category_from_str_and_all() {
        assert_eq!(
            CleanCategory::from_str("package").expect("parse package"),
            CleanCategory::PackageCache
        );
        assert_eq!(
            CleanCategory::from_str("pkg").expect("parse pkg"),
            CleanCategory::PackageCache
        );
        assert_eq!(
            CleanCategory::from_str("package-cache").expect("parse package-cache"),
            CleanCategory::PackageCache
        );

        assert_eq!(
            CleanCategory::from_str("artifacts").expect("parse artifacts"),
            CleanCategory::Artifacts
        );
        assert_eq!(
            CleanCategory::from_str("build").expect("parse build"),
            CleanCategory::Artifacts
        );

        assert_eq!(
            CleanCategory::from_str("thumbnails").expect("parse thumbnails"),
            CleanCategory::Thumbnails
        );
        assert_eq!(
            CleanCategory::from_str("thumbs").expect("parse thumbs"),
            CleanCategory::Thumbnails
        );

        assert_eq!(
            CleanCategory::from_str("trash").expect("parse trash"),
            CleanCategory::Trash
        );
        assert_eq!(
            CleanCategory::from_str("user_trash").expect("parse user_trash"),
            CleanCategory::Trash
        );

        assert_eq!(
            CleanCategory::from_str("backups").expect("parse backups"),
            CleanCategory::Backups
        );
        assert_eq!(
            CleanCategory::from_str("dotfiles-backups").expect("parse dotfiles-backups"),
            CleanCategory::Backups
        );

        assert_eq!(
            CleanCategory::from_str("logs").expect("parse logs"),
            CleanCategory::LogsCache
        );
        assert_eq!(
            CleanCategory::from_str("cache").expect("parse cache"),
            CleanCategory::LogsCache
        );
        assert_eq!(
            CleanCategory::from_str("Logs & Cache").expect("parse Logs & Cache"),
            CleanCategory::LogsCache
        );

        assert_eq!(
            CleanCategory::from_str("hotspots").expect("parse hotspots"),
            CleanCategory::Hotspots
        );
        assert_eq!(
            CleanCategory::from_str("disk_hotspots").expect("parse disk_hotspots"),
            CleanCategory::Hotspots
        );

        assert!(CleanCategory::from_str("unknown_category").is_err());

        let all_cats = CleanCategory::all();
        assert_eq!(all_cats.len(), 7);
        assert!(all_cats.contains(&CleanCategory::PackageCache));
        assert!(all_cats.contains(&CleanCategory::Artifacts));
        assert!(all_cats.contains(&CleanCategory::Thumbnails));
        assert!(all_cats.contains(&CleanCategory::Trash));
        assert!(all_cats.contains(&CleanCategory::Backups));
        assert!(all_cats.contains(&CleanCategory::LogsCache));
        assert!(all_cats.contains(&CleanCategory::Hotspots));
    }
}
