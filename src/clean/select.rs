// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Interactive grouped target selection and fault-tolerant clean execution

use crate::clean::classifier::CleanTarget;
use crate::clean::privilege::is_elevated;
use crate::clean::walker::CleanReport;
use humansize::{DECIMAL, format_size};
use inquire::MultiSelect;
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

/// Result summary of clean execution.
#[derive(Debug, Default)]
pub struct DeletionSummary {
    pub deleted_targets: usize,
    pub reclaimed_bytes: u64,
    pub skipped_privilege: Vec<CleanTarget>,
    pub failures: Vec<(PathBuf, String)>,
}

/// Executes interactive target selection and fault-tolerant deletion.
pub fn interactive_select_and_clean(report: &CleanReport) -> Result<(usize, u64), CleanError> {
    if report.targets.is_empty() {
        println!("No cleanable targets found.");
        return Ok((0, 0));
    }

    let options: Vec<TargetOption> = report
        .targets
        .iter()
        .enumerate()
        .map(|(index, target)| TargetOption {
            index,
            target: target.clone(),
        })
        .collect();

    // Selection itself is the user commit (ADR §2.3 / scoping review)
    let prompt = "Select cleanup targets (Space to toggle, Enter to confirm & delete):";
    let chosen_options = MultiSelect::new(prompt, options)
        .prompt()
        .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

    if chosen_options.is_empty() {
        println!("No targets selected. Exiting without changes.");
        return Ok((0, 0));
    }

    let chosen_targets: Vec<&CleanTarget> = chosen_options
        .iter()
        .map(|opt| &report.targets[opt.index])
        .collect();

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
            "✓ Successfully cleaned {} target(s), reclaiming {}.",
            summary.deleted_targets,
            format_size(summary.reclaimed_bytes, DECIMAL)
        );
    }

    if !summary.skipped_privilege.is_empty() {
        let total_skipped_size: u64 = summary.skipped_privilege.iter().map(|t| t.size_bytes).sum();
        println!(
            "\n⚠ Skipped {} target(s) ({}) requiring root privileges (rerun with 'sudo voidctl clean select'):",
            summary.skipped_privilege.len(),
            format_size(total_skipped_size, DECIMAL)
        );
        for t in &summary.skipped_privilege {
            println!("  - {} ({})", t.title, t.path.display());
        }
    }

    if !summary.failures.is_empty() {
        println!("\n⚠ Encountered error(s) during deletion:");
        for (path, err) in &summary.failures {
            eprintln!("  - Failed to delete '{}': {}", path.display(), err);
        }
    }
}
