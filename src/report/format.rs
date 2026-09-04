// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Human-readable terminal table rendering, UTF-8 path truncation, size formatting, and colored CLI reporting

use crate::clean::classifier::CleanCategory;
use crate::clean::walker::CleanReport;
use crate::drift::DriftAuditReport;
use crate::drift::links::{LinkRecord, LinkStatus};
use colored::{ColoredString, Colorize};
use humansize::{DECIMAL, format_size};
use std::collections::HashMap;
use std::path::Path;

/// Formats and prints a clean scan report.
pub fn print_clean_report(report: &CleanReport, show_all: bool) {
    if report.targets.is_empty() && report.items.is_empty() {
        println!("{}", "No reclaimable targets found.".green());
        print_clean_footer(report);
        return;
    }

    println!(
        "{:<16} {:<36} {:>8} {:>10}  {}",
        "CATEGORY".bold().cyan(),
        "TARGET".bold().cyan(),
        "COUNT".bold().cyan(),
        "SIZE".bold().cyan(),
        "NOTES".bold().cyan()
    );
    println!("{}", "-".repeat(105).dimmed());

    if !report.targets.is_empty() {
        let max_display = 25;
        let should_truncate = !show_all && report.targets.len() > max_display;
        let displayed_targets = if should_truncate {
            &report.targets[..max_display]
        } else {
            &report.targets[..]
        };

        for target in displayed_targets {
            let size_str = colorize_size(target.size_bytes);
            let title_str = truncate_path(&target.title, 36);
            let count_str = if target.item_count > 1 {
                target.item_count.to_string()
            } else if target.is_tree {
                "tree".to_string()
            } else {
                "1".to_string()
            };
            let root_notice = if target.requires_elevation {
                " [root]".bold().red()
            } else {
                "".normal()
            };
            let category_str = colorize_category(target.category);
            println!(
                "{:<16} {:<36} {:>8} {:>10}  {}{}",
                category_str,
                title_str.bold(),
                count_str.dimmed(),
                size_str,
                target.reason,
                root_notice
            );
        }

        if should_truncate {
            let remaining_count = report.targets.len() - max_display;
            let remaining_size: u64 = report.targets[max_display..]
                .iter()
                .map(|t| t.size_bytes)
                .sum();
            let remaining_items: usize = report.targets[max_display..]
                .iter()
                .map(|t| t.item_count)
                .sum();
            let count_str = if remaining_items > 0 {
                remaining_items.to_string()
            } else {
                "tree".to_string()
            };
            println!(
                "{:<16} {:<36} {:>8} {:>10}  Run 'voidctl clean scan --all' to list all",
                "...".dimmed(),
                format!("... and {remaining_count} more targets").dimmed(),
                count_str.dimmed(),
                colorize_size(remaining_size)
            );
        }
    } else {
        for item in &report.items {
            let size_str = colorize_size(item.size_bytes);
            let path_str = truncate_path(&item.path.to_string_lossy(), 36);
            let category_str = colorize_category(item.category);
            println!(
                "{:<16} {:<36} {:>8} {:>10}  {}",
                category_str,
                path_str.bold(),
                if item.is_dir {
                    "tree".dimmed()
                } else {
                    "1".dimmed()
                },
                size_str,
                item.reason
            );
        }
    }

    println!("{}", "-".repeat(105).dimmed());
    print_clean_footer(report);
}

/// Prints footer information for clean scan including total size, location breakdown, top contributors, and privilege notices.
fn print_clean_footer(report: &CleanReport) {
    let total_bytes = report.total_reclaimable_bytes();
    let total_size_str = format_size(total_bytes, DECIMAL);
    let target_count = if !report.targets.is_empty() {
        report.targets.len()
    } else {
        report.items.len()
    };
    let total_items = report.total_item_count();

    println!(
        "\n{} {} ({} cleanup target(s), {} total item(s))",
        "Total reclaimable space:".bold(),
        total_size_str.bold().green(),
        target_count.to_string().bold(),
        total_items.to_string().dimmed()
    );

    if !report.targets.is_empty() {
        // 1. Top 4 largest targets breakdown
        let top_n = report.targets.len().min(4);
        let top_targets = &report.targets[..top_n];
        let top_combined_size: u64 = top_targets.iter().map(|t| t.size_bytes).sum();
        let top_combined_pct = if total_bytes > 0 {
            (top_combined_size as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        println!("\n{}", "Top Contributors:".bold().cyan());
        for (i, t) in top_targets.iter().enumerate() {
            let pct = if total_bytes > 0 {
                (t.size_bytes as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  {}. {:<32} {:>10}  {}",
                i + 1,
                truncate_path(&t.title, 32).bold(),
                colorize_size(t.size_bytes),
                format!("({pct:.1}%)").dimmed()
            );
        }
        println!(
            "  {} Top {} account for {} ({} of reclaimable space)",
            "↳".dimmed(),
            top_n,
            format_size(top_combined_size, DECIMAL).bold(),
            format!("{top_combined_pct:.1}%").bold().yellow()
        );

        // 2. Classification by filesystem / location root (/home vs /)
        let mut home_bytes: u64 = 0;
        let mut home_items: usize = 0;
        let mut root_bytes: u64 = 0;
        let mut root_items: usize = 0;

        for t in &report.targets {
            if is_home_path(&t.path) {
                home_bytes += t.size_bytes;
                home_items += t.item_count;
            } else {
                root_bytes += t.size_bytes;
                root_items += t.item_count;
            }
        }

        println!("\n{}", "Reclaimable by Location:".bold().cyan());
        if home_bytes > 0 {
            let pct = (home_bytes as f64 / total_bytes as f64) * 100.0;
            println!(
                "  • {:<20} {:>10}  {}",
                "Home (~/):".bold(),
                colorize_size(home_bytes),
                format!("({pct:.1}%, {} items)", home_items).dimmed()
            );
        }
        if root_bytes > 0 {
            let pct = (root_bytes as f64 / total_bytes as f64) * 100.0;
            println!(
                "  • {:<20} {:>10}  {}",
                "System Root (/):".bold(),
                colorize_size(root_bytes),
                format!("({pct:.1}%, {} items)", root_items).dimmed()
            );
        }

        // 3. Classification by Category
        let mut cat_map: HashMap<CleanCategory, (u64, usize)> = HashMap::new();
        for t in &report.targets {
            let entry = cat_map.entry(t.category).or_insert((0, 0));
            entry.0 += t.size_bytes;
            entry.1 += 1;
        }
        let mut cat_vec: Vec<(CleanCategory, u64, usize)> = cat_map
            .into_iter()
            .map(|(cat, (size, count))| (cat, size, count))
            .collect();
        cat_vec.sort_by_key(|c| std::cmp::Reverse(c.1));

        println!("\n{}", "Reclaimable by Category:".bold().cyan());
        for (cat, size, count) in cat_vec {
            let pct = if total_bytes > 0 {
                (size as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  • {:<20} {:>10}  {}",
                colorize_category(cat),
                colorize_size(size),
                format!("({pct:.1}%, {count} target(s))").dimmed()
            );
        }
    }

    let has_root_targets = report.targets.iter().any(|t| t.requires_elevation);
    if report.skipped_permission_denied > 0 || has_root_targets {
        println!(
            "\n{}",
            "Notice: Some targets require root privileges — rerun under sudo ('sudo voidctl clean') to clean them."
                .yellow()
        );
    }
}

/// Checks if a given path resides inside the user home directory.
fn is_home_path(path: &Path) -> bool {
    if let Some(home) = crate::config::resolve_home_dir()
        && path.starts_with(&home)
    {
        return true;
    }
    path.starts_with("/home")
}

/// Colorizes a category enum for terminal presentation.
fn colorize_category(cat: CleanCategory) -> ColoredString {
    match cat {
        CleanCategory::PackageCache => cat.to_string().yellow(),
        CleanCategory::LogsCache => cat.to_string().blue(),
        CleanCategory::Artifacts => cat.to_string().magenta(),
        CleanCategory::Thumbnails => cat.to_string().green(),
        CleanCategory::Trash => cat.to_string().red(),
        CleanCategory::Backups => cat.to_string().purple(),
        CleanCategory::Hotspots => cat.to_string().bright_red().bold(),
    }
}

/// Colorizes size strings according to magnitude.
fn colorize_size(size_bytes: u64) -> ColoredString {
    let s = format_size(size_bytes, DECIMAL);
    if size_bytes >= 1_000_000_000 {
        s.bold().yellow()
    } else if size_bytes >= 100_000_000 {
        s.yellow()
    } else {
        s.normal()
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
    println!(
        "{:<22} {:<32}  {}",
        "STATUS".bold().cyan(),
        "TARGET".bold().cyan(),
        "SOURCE".bold().cyan()
    );
    println!("{}", "-".repeat(80).dimmed());

    for rec in records {
        let status_str = rec.status.to_string();
        let status_colored = match &rec.status {
            LinkStatus::Valid => status_str.green(),
            LinkStatus::Broken { .. } => status_str.bold().red(),
            LinkStatus::ReplacedByRealFile => status_str.magenta(),
            LinkStatus::PermissionDrift { .. } => status_str.yellow(),
            LinkStatus::Missing | LinkStatus::MissingSource => status_str.red(),
        };
        println!(
            "{:<22} {:<32}  {}",
            status_colored,
            rec.target_rel,
            rec.source_rel.dimmed()
        );
    }
}

/// Prints git repository status entries.
pub fn print_git_records(git_entries: &Result<Vec<crate::drift::GitStatusEntry>, String>) {
    match git_entries {
        Ok(entries) => {
            if entries.is_empty() {
                println!(
                    "{}",
                    "Repository working tree is clean (no uncommitted changes).".green()
                );
            } else {
                println!("{:<8} {}", "STATE".bold().cyan(), "FILE".bold().cyan());
                println!("{}", "-".repeat(50).dimmed());
                for entry in entries {
                    let state = format!("{}{}", entry.index_state, entry.worktree_state);
                    let state_colored = if state.contains('M') {
                        state.yellow()
                    } else if state.contains('A') {
                        state.green()
                    } else if state.contains('D') {
                        state.red()
                    } else {
                        state.dimmed()
                    };
                    println!("{:<8} {}", state_colored, entry.path);
                }
            }
        }
        Err(err) => println!(
            "{}: {}",
            "Could not query dotfiles git repository".red(),
            err
        ),
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
