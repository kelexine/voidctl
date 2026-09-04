// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Human-readable terminal table rendering and size formatting

use crate::clean::walker::CleanReport;
use crate::drift::DriftAuditReport;
use crate::drift::links::LinkRecord;
use humansize::{DECIMAL, format_size};

/// Formats and prints a clean scan report.
pub fn print_clean_report(report: &CleanReport) {
    if report.items.is_empty() {
        println!("No reclaimable targets found.");
        print_clean_footer(report);
        return;
    }

    println!(
        "{:<17} {:<55} {:>10}  REASON",
        "CATEGORY", "TARGET PATH", "SIZE"
    );
    println!("{}", "-".repeat(105));

    for item in &report.items {
        let size_str = format_size(item.size_bytes, DECIMAL);
        let path_str = truncate_path(&item.path.to_string_lossy(), 55);
        println!(
            "{:<17} {:<55} {:>10}  {}",
            item.category.to_string(),
            path_str,
            size_str,
            item.reason
        );
    }

    println!("{}", "-".repeat(105));
    print_clean_footer(report);
}

/// Prints footer information for clean scan including total size and privilege notices.
fn print_clean_footer(report: &CleanReport) {
    let total_size = format_size(report.total_reclaimable_bytes(), DECIMAL);
    println!(
        "Total reclaimable space: {} ({} items)",
        total_size,
        report.items.len()
    );

    if report.skipped_permission_denied > 0 {
        println!(
            "Notice: {} paths skipped (permission denied) — rerun under sudo to include them.",
            report.skipped_permission_denied
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

/// Truncates a long path with ellipsis for table display.
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        let suffix_len = max_len.saturating_sub(3);
        format!("...{}", &path[path.len() - suffix_len..])
    }
}
