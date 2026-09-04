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
