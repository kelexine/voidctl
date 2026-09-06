// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Filesystem directory sizing, item counting, and human-readable metric parsing

use std::fs;
use std::path::Path;

/// Recursively computes directory size in bytes.
#[must_use]
pub fn calculate_dir_size(path: &Path) -> u64 {
    let (size, _) = count_and_size_dir(path);
    size
}

/// Computes total size and item count within a directory tree.
#[must_use]
pub fn count_and_size_dir(path: &Path) -> (u64, usize) {
    let mut total_size: u64 = 0;
    let mut count: usize = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let (sub_size, sub_count) = count_and_size_dir(&p);
                    total_size += sub_size;
                    count += sub_count;
                } else {
                    total_size += meta.len();
                    count += 1;
                }
            }
        }
    }

    (total_size, count)
}

/// Parses human size string (e.g. "52.4M", "1.2G", "800K") into bytes.
#[must_use]
pub fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let num_end = s.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let (num_part, unit_part) = s.split_at(num_end);
    let val: f64 = num_part.parse().ok()?;
    let unit = unit_part.to_uppercase();

    let multiplier = if unit.starts_with('K') {
        1024.0
    } else if unit.starts_with('M') {
        1024.0 * 1024.0
    } else if unit.starts_with('G') {
        1024.0 * 1024.0 * 1024.0
    } else if unit.starts_with('T') {
        1024.0 * 1024.0 * 1024.0 * 1024.0
    } else {
        1.0
    };

    Some((val * multiplier) as u64)
}

/// Parses the output of `journalctl --disk-usage`.
#[must_use]
pub fn parse_journal_disk_usage(output: &str) -> Option<u64> {
    let take_up = output.find("take up ")?;
    let rest = &output[take_up + "take up ".len()..];
    let size_token = rest.split_whitespace().next()?;
    parse_human_size(size_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_human_size_and_journal_output() {
        assert_eq!(parse_human_size("1024K"), Some(1024 * 1024));
        assert_eq!(parse_human_size("50M"), Some(50 * 1024 * 1024));
        assert_eq!(parse_human_size("2G"), Some(2 * 1024 * 1024 * 1024));

        let sample = "Archived and active journals take up 52.4M in the file system.";
        let parsed = parse_journal_disk_usage(sample).expect("parsed journal disk usage");
        assert!(parsed > 50 * 1024 * 1024);
    }
}
