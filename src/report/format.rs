// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Human-readable terminal table rendering, UTF-8 path truncation, and size formatting

use crate::clean::walker::CleanReport;
use crate::drift::DriftAuditReport;
use crate::drift::links::LinkRecord;
use humansize::{DECIMAL, format_size};

/// Formats and prints a clean scan report.
pub fn print_clean_report(report: &CleanReport) {
    if report.targets.is_empty() && report.items.is_empty() {
        println!("No reclaimable targets found.");
        print_clean_footer(report);
        return;
    }

    println!(
        "{:<16} {:<36} {:>8} {:>10}  NOTES",
        "CATEGORY", "TARGET", "COUNT", "SIZE"
    );
    println!("{}", "-".repeat(105));

    if !report.targets.is_empty() {
        for target in &report.targets {
            let size_str = format_size(target.size_bytes, DECIMAL);
            let title_str = truncate_path(&target.title, 36);
            let count_str = if target.item_count > 1 {
                target.item_count.to_string()
            } else if target.is_tree {
                "tree".to_string()
            } else {
                "1".to_string()
            };
            let root_notice = if target.requires_elevation {
                " [root]"
            } else {
                ""
            };
            let notes = format!("{}{}", target.reason, root_notice);
            println!(
                "{:<16} {:<36} {:>8} {:>10}  {}",
                target.category.to_string(),
                title_str,
                count_str,
                size_str,
                notes
            );
        }
    } else {
        for item in &report.items {
            let size_str = format_size(item.size_bytes, DECIMAL);
            let path_str = truncate_path(&item.path.to_string_lossy(), 36);
            println!(
                "{:<16} {:<36} {:>8} {:>10}  {}",
                item.category.to_string(),
                path_str,
                if item.is_dir { "tree" } else { "1" },
                size_str,
                item.reason
            );
        }
    }

    println!("{}", "-".repeat(105));
    print_clean_footer(report);
}

/// Prints footer information for clean scan including total size and privilege notices.
fn print_clean_footer(report: &CleanReport) {
    let total_size = format_size(report.total_reclaimable_bytes(), DECIMAL);
    let target_count = if !report.targets.is_empty() {
        report.targets.len()
    } else {
        report.items.len()
    };
    let total_items = report.total_item_count();

    println!(
        "Total reclaimable space: {} ({} cleanup target(s), {} total item(s))",
        total_size, target_count, total_items
    );

    let has_root_targets = report.targets.iter().any(|t| t.requires_elevation);
    if report.skipped_permission_denied > 0 || has_root_targets {
        println!(
            "Notice: Some targets require root privileges — rerun under sudo ('sudo voidctl clean') to clean them."
        );
    }
}

/// Formats and prints a dotfiles drift audit report.
pub fn print_drift_report(report: &DriftAuditReport) {
    print_symlink_records(&report.link_records);
    println!();
    print_git_records(&report.git_entries);
}

/// Prints formatted symlink records table.
pub fn print_symlink_records(records: &[LinkRecord]) {
    println!("{:<22} {:<32}  SOURCE", "STATUS", "TARGET");
    println!("{}", "-".repeat(80));

    for rec in records {
        println!(
            "{:<22} {:<32}  {}",
            rec.status.to_string(),
            rec.target_rel,
            rec.source_rel
        );
    }
}

/// Prints git repository status entries.
pub fn print_git_records(git_entries: &Result<Vec<crate::drift::GitStatusEntry>, String>) {
    match git_entries {
        Ok(entries) => {
            if entries.is_empty() {
                println!("Repository working tree is clean (no uncommitted changes).");
            } else {
                println!("{:<8} FILE", "STATE");
                println!("{}", "-".repeat(50));
                for entry in entries {
                    let state = format!("{}{}", entry.index_state, entry.worktree_state);
                    println!("{:<8} {}", state, entry.path);
                }
            }
        }
        Err(err) => println!("Could not query dotfiles git repository: {}", err),
    }
}

/// Truncates a string with ellipsis safely respecting UTF-8 character boundaries.
#[must_use]
pub fn truncate_path(path: &str, max_len: usize) -> String {
    let char_count = path.chars().count();
    if char_count <= max_len {
        path.to_string()
    } else {
        let skip_chars = char_count - max_len.saturating_sub(3);
        let byte_offset = path
            .char_indices()
            .nth(skip_chars)
            .map(|(idx, _)| idx)
            .unwrap_or(path.len());
        format!("...{}", &path[byte_offset..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_path_ascii() {
        assert_eq!(truncate_path("short_path", 20), "short_path");
        assert_eq!(
            truncate_path("/very/long/path/to/something", 15),
            "...to/something"
        );
    }

    #[test]
    fn test_truncate_path_multibyte_utf8() {
        // Multi-byte Unicode, Japanese, Emoji
        let text = "🦀🚀_rust_special_project_path_file.rs";
        let truncated = truncate_path(text, 15);
        assert!(truncated.starts_with("..."));
        // Must be valid UTF-8 and not panic
        assert_eq!(truncated.chars().count(), 15);
    }
}
