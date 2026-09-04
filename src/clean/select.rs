// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Interactive grouped target selection, category selection, and fault-tolerant clean execution

use crate::clean::classifier::{CleanCategory, CleanTarget};
use crate::clean::privilege::is_elevated;
use crate::clean::walker::CleanReport;
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use inquire::{MultiSelect, Select};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Error conditions during interactive selection or deletion.
#[derive(Debug, Error)]
pub enum CleanError {
    #[error("Interactive prompt aborted: {0}")]
    PromptAborted(String),
}

/// Helper display wrapper for grouped cleanup targets in multi-select prompt.
#[derive(Clone, PartialEq, Eq)]
struct TargetOption {
    index: usize,
    target: CleanTarget,
}

impl fmt::Display for TargetOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size_str = format_size(self.target.size_bytes, DECIMAL);
        let elev_notice = if self.target.requires_elevation {
            " [Requires Root]"
        } else {
            ""
        };
        let count_desc = if self.target.item_count > 1 {
            format!(" ({} items)", self.target.item_count)
        } else {
            String::new()
        };

        write!(
            f,
            "[{}] {} — {}{}{}",
            self.target.category, self.target.title, size_str, count_desc, elev_notice
        )
    }
}

/// Helper display wrapper for whole cleanup categories in multi-select prompt.
#[derive(Clone, PartialEq, Eq)]
struct CategoryOption {
    category: CleanCategory,
    target_count: usize,
    total_size: u64,
    item_count: usize,
    has_root: bool,
}

impl fmt::Display for CategoryOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size_str = format_size(self.total_size, DECIMAL);
        let root_notice = if self.has_root {
            " [Requires Root]"
        } else {
            ""
        };
        write!(
            f,
            "[{}] {} across {} target(s) ({} item(s)){}",
            self.category, size_str, self.target_count, self.item_count, root_notice
        )
    }
}

/// Result summary of clean execution.
#[derive(Debug, Default)]
pub struct DeletionSummary {
    pub deleted_targets: usize,
    pub reclaimed_bytes: u64,
    pub skipped_privilege: Vec<CleanTarget>,
    pub failures: Vec<(PathBuf, String)>,
}

/// Executes interactive target selection and fault-tolerant deletion.
pub fn interactive_select_and_clean(
    report: &CleanReport,
    specific_category: Option<CleanCategory>,
) -> Result<(usize, u64), CleanError> {
    if report.targets.is_empty() {
        println!("{}", "No cleanable targets found.".green());
        return Ok((0, 0));
    }

    let elevated = is_elevated();

    let chosen_targets: Vec<&CleanTarget> = if let Some(cat) = specific_category {
        let matching: Vec<&CleanTarget> = report
            .targets
            .iter()
            .filter(|t| t.category == cat)
            .collect();

        if matching.is_empty() {
            println!(
                "{}",
                format!("No cleanable targets found under category '{cat}'.").yellow()
            );
            return Ok((0, 0));
        }

        let cat_size: u64 = matching.iter().map(|t| t.size_bytes).sum();
        let cat_count: usize = matching.iter().map(|t| t.item_count).sum();
        println!(
            "{}",
            format!(
                "Category '{}' contains {} target(s) totaling {} ({} item(s)).",
                cat,
                matching.len(),
                format_size(cat_size, DECIMAL),
                cat_count
            )
            .bold()
            .cyan()
        );

        let options: Vec<TargetOption> = matching
            .iter()
            .enumerate()
            .map(|(index, target)| TargetOption {
                index,
                target: (*target).clone(),
            })
            .collect();

        let prompt =
            format!("Select targets in '{cat}' to delete (Space to toggle, Enter to delete):");
        let chosen = MultiSelect::new(&prompt, options)
            .prompt()
            .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

        if chosen.is_empty() {
            println!("No targets selected. Exiting without changes.");
            return Ok((0, 0));
        }

        chosen.into_iter().map(|opt| matching[opt.index]).collect()
    } else {
        // Prompt for selection scope
        let scope = Select::new(
            "How would you like to select items to clean?",
            vec![
                "By Category (bulk clean entire categories like Package Cache, Thumbnails)",
                "By Target (select specific directories and caches individually)",
                "All Safe Targets (clean all targets that do not require root)",
            ],
        )
        .prompt()
        .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

        if scope.starts_with("By Category") {
            let mut cat_map: HashMap<CleanCategory, (usize, u64, usize, bool)> = HashMap::new();
            for t in &report.targets {
                let entry = cat_map.entry(t.category).or_insert((0, 0, 0, false));
                entry.0 += 1;
                entry.1 += t.size_bytes;
                entry.2 += t.item_count;
                if t.requires_elevation {
                    entry.3 = true;
                }
            }

            let mut cat_options: Vec<CategoryOption> = cat_map
                .into_iter()
                .map(|(cat, (count, size, items, has_root))| CategoryOption {
                    category: cat,
                    target_count: count,
                    total_size: size,
                    item_count: items,
                    has_root,
                })
                .collect();
            cat_options.sort_by_key(|c| std::cmp::Reverse(c.total_size));

            let chosen_cats = MultiSelect::new(
                "Select categories to clean (Space to toggle, Enter to confirm & delete):",
                cat_options,
            )
            .prompt()
            .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

            if chosen_cats.is_empty() {
                println!("No categories selected. Exiting without changes.");
                return Ok((0, 0));
            }

            let selected_set: HashSet<CleanCategory> =
                chosen_cats.into_iter().map(|c| c.category).collect();

            report
                .targets
                .iter()
                .filter(|t| selected_set.contains(&t.category))
                .collect()
        } else if scope.starts_with("By Target") {
            let options: Vec<TargetOption> = report
                .targets
                .iter()
                .enumerate()
                .map(|(index, target)| TargetOption {
                    index,
                    target: target.clone(),
                })
                .collect();

            let prompt = "Select cleanup targets (Space to toggle, Enter to confirm & delete):";
            let chosen_options = MultiSelect::new(prompt, options)
                .prompt()
                .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

            if chosen_options.is_empty() {
                println!("No targets selected. Exiting without changes.");
                return Ok((0, 0));
            }

            chosen_options
                .iter()
                .map(|opt| &report.targets[opt.index])
                .collect()
        } else {
            // All safe targets
            let safe_targets: Vec<&CleanTarget> = report
                .targets
                .iter()
                .filter(|t| !t.requires_elevation || elevated)
                .collect();

            if safe_targets.is_empty() {
                println!(
                    "{}",
                    "All available targets require root privileges. Rerun under sudo.".yellow()
                );
                return Ok((0, 0));
            }

            let total_safe: u64 = safe_targets.iter().map(|t| t.size_bytes).sum();
            println!(
                "{}",
                format!(
                    "Cleaning all {} safe targets totaling {}...",
                    safe_targets.len(),
                    format_size(total_safe, DECIMAL)
                )
                .bold()
                .cyan()
            );
            safe_targets
        }
    };

    let summary = execute_deletions(&chosen_targets);
    print_deletion_summary(&summary);

    Ok((summary.deleted_targets, summary.reclaimed_bytes))
}

/// Fault-tolerant deletion loop executing on selected targets.
fn execute_deletions(targets: &[&CleanTarget]) -> DeletionSummary {
    let mut summary = DeletionSummary::default();
    let elevated = is_elevated();

    for target in targets {
        if target.requires_elevation && !elevated {
            summary.skipped_privilege.push((*target).clone());
            continue;
        }

        let mut target_deleted = false;
        let mut target_bytes = 0;

        if target.is_tree {
            if target.path.exists() {
                match fs::remove_dir_all(&target.path) {
                    Ok(()) => {
                        target_deleted = true;
                        target_bytes = target.size_bytes;
                    }
                    Err(err) => {
                        summary
                            .failures
                            .push((target.path.clone(), err.to_string()));
                    }
                }
            }
        } else if !target.files.is_empty() {
            let mut file_failures = 0;
            let mut deleted_file_bytes = 0;

            for file in &target.files {
                if file.exists() {
                    let file_size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
                    match fs::remove_file(file) {
                        Ok(()) => {
                            deleted_file_bytes += file_size;
                        }
                        Err(err) => {
                            file_failures += 1;
                            if file_failures <= 3 {
                                summary.failures.push((file.clone(), err.to_string()));
                            }
                        }
                    }
                }
            }

            if deleted_file_bytes > 0 {
                target_deleted = true;
                target_bytes = deleted_file_bytes;
            }
        } else if target.path.exists() {
            let res = if target.path.is_dir() {
                fs::remove_dir_all(&target.path)
            } else {
                fs::remove_file(&target.path)
            };

            match res {
                Ok(()) => {
                    target_deleted = true;
                    target_bytes = target.size_bytes;
                }
                Err(err) => {
                    summary
                        .failures
                        .push((target.path.clone(), err.to_string()));
                }
            }
        }

        if target_deleted {
            summary.deleted_targets += 1;
            summary.reclaimed_bytes += target_bytes;
        }
    }

    summary
}

/// Prints formatted outcome of deletion operations.
fn print_deletion_summary(summary: &DeletionSummary) {
    if summary.deleted_targets > 0 {
        println!(
            "\n{} Successfully cleaned {} target(s), reclaiming {}.",
            "✓".bold().green(),
            summary.deleted_targets.to_string().bold(),
            format_size(summary.reclaimed_bytes, DECIMAL).bold().green()
        );
    }

    if !summary.skipped_privilege.is_empty() {
        let total_skipped_size: u64 = summary.skipped_privilege.iter().map(|t| t.size_bytes).sum();
        println!(
            "\n{} Skipped {} target(s) ({}) requiring root privileges (rerun with 'sudo voidctl clean select'):",
            "⚠".bold().yellow(),
            summary.skipped_privilege.len(),
            format_size(total_skipped_size, DECIMAL)
        );
        for t in &summary.skipped_privilege {
            println!(
                "  - {} ({})",
                t.title.bold(),
                t.path.display().to_string().dimmed()
            );
        }
    }

    if !summary.failures.is_empty() {
        println!(
            "\n{} Encountered error(s) during deletion:",
            "⚠".bold().red()
        );
        for (path, err) in &summary.failures {
            eprintln!("  - Failed to delete '{}': {}", path.display(), err.red());
        }
    }
}
