// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Classifier for aged logs and temporary cache trees

use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

/// Number of seconds in a standard 24-hour day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Determines if a file is a log file that exceeds the age threshold.
#[must_use]
pub fn is_stale_log_file(path: &Path, metadata: &Metadata, threshold_days: u64) -> bool {
    let is_log = path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("log"));

    if !is_log {
        return false;
    }

    is_older_than(metadata, threshold_days)
}

/// Checks if an entry is located in a system temporary tree and exceeds the age threshold.
#[must_use]
pub fn is_stale_cache_entry(path: &Path, metadata: &Metadata, threshold_days: u64) -> bool {
    let path_str = path.to_string_lossy();
    let is_temp_tree = path_str.starts_with("/tmp/") || path_str.starts_with("/var/tmp/");

    if !is_temp_tree {
        return false;
    }

    is_older_than(metadata, threshold_days)
}

/// Helper function to check if file modification time exceeds the given day threshold.
#[must_use]
pub fn is_older_than(metadata: &Metadata, threshold_days: u64) -> bool {
    let mtime = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };

    let elapsed = match SystemTime::now().duration_since(mtime) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };

    elapsed >= (threshold_days * SECONDS_PER_DAY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_fresh_log_not_stale() {
        let dir = tempdir().expect("tempdir");
        let log_file = dir.path().join("app.log");
        let file = File::create(&log_file).expect("create file");
        let metadata = file.metadata().expect("metadata");

        assert!(!is_stale_log_file(&log_file, &metadata, 7));
    }
}
