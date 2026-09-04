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
    /// Build artifacts (e.g. target/, node_modules/, __pycache__/).
    Artifacts,
    /// Aged log files and cache hierarchies (>7 days).
    LogsCache,
    /// High-capacity storage consumers.
    Hotspots,
}

impl fmt::Display for CleanCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifacts => write!(f, "Build Artifacts"),
            Self::LogsCache => write!(f, "Logs & Cache"),
            Self::Hotspots => write!(f, "Disk Hotspots"),
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
