// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Disk hotspot tracking for identifying large individual files and trees

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

/// Maximum number of hotspot candidates retained in memory.
pub const MAX_HOTSPOTS: usize = 20;

/// Represents an individual disk hotspot item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotspotItem {
    pub path: PathBuf,
    pub size_bytes: u64,
}

impl Ord for HotspotItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.size_bytes.cmp(&other.size_bytes)
    }
}

impl PartialOrd for HotspotItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Tracks top-N largest files visited during the filesystem walk.
#[derive(Debug, Default, Clone)]
pub struct HotspotTracker {
    heap: BinaryHeap<Reverse<HotspotItem>>,
    capacity: usize,
}

impl HotspotTracker {
    /// Creates a tracker with default hotspot capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity),
            capacity,
        }
    }

    /// Observes a path and its size, retaining it if among top-N.
    pub fn observe(&mut self, path: &Path, size_bytes: u64) {
        let item = HotspotItem {
            path: path.to_path_buf(),
            size_bytes,
        };

        if self.heap.len() < self.capacity {
            self.heap.push(Reverse(item));
        } else if let Some(min) = self.heap.peek()
            && size_bytes > min.0.size_bytes
        {
            self.heap.pop();
            self.heap.push(Reverse(item));
        }
    }

    /// Consumes the tracker and returns items sorted by size descending.
    #[must_use]
    pub fn into_sorted_vec(self) -> Vec<HotspotItem> {
        let mut items: Vec<HotspotItem> = self.heap.into_iter().map(|Reverse(i)| i).collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.size_bytes));
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotspot_tracker() {
        let mut tracker = HotspotTracker::new(2);
        tracker.observe(Path::new("/file1"), 100);
        tracker.observe(Path::new("/file2"), 500);
        tracker.observe(Path::new("/file3"), 300);

        let items = tracker.into_sorted_vec();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].size_bytes, 500);
        assert_eq!(items[1].size_bytes, 300);
    }
}
