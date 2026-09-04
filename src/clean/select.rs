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

/// Deletes a whole directory tree target.
fn delete_tree_target(target: &CleanTarget, summary: &mut DeletionSummary) -> bool {
    if target.path.exists() {
        match fs::remove_dir_all(&target.path) {
            Ok(()) => true,
            Err(err) => {
                summary
                    .failures
                    .push((target.path.clone(), err.to_string()));
                false
            }
        }
    } else {
        summary.failures.push((
            target.path.clone(),
            "Target directory does not exist".to_string(),
        ));
        false
    }
}

/// Deletes a collection of specific files within a target.
fn delete_files_target(target: &CleanTarget, summary: &mut DeletionSummary) -> u64 {
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
        } else {
            file_failures += 1;
            if file_failures <= 3 {
                summary
                    .failures
                    .push((file.clone(), "File does not exist".to_string()));
            }
        }
    }

    deleted_file_bytes
}

/// Deletes a single target path (file or directory).
fn delete_single_path_target(target: &CleanTarget, summary: &mut DeletionSummary) -> bool {
    if !target.path.exists() {
        summary.failures.push((
            target.path.clone(),
            "Target path does not exist".to_string(),
        ));
        return false;
    }

    if target.path == std::path::Path::new("/var/log/journal") {
        match std::process::Command::new("journalctl")
            .args(["--vacuum-time=14d"])
            .status()
        {
            Ok(status) if status.success() => return true,
            Ok(status) => {
                summary.failures.push((
                    target.path.clone(),
                    format!("journalctl --vacuum-time=14d exited with {status}"),
                ));
                return false;
            }
            Err(err) => {
                summary
                    .failures
                    .push((target.path.clone(), err.to_string()));
                return false;
            }
        }
    }

    let res = if target.path.is_dir() {
        fs::remove_dir_all(&target.path)
    } else {
        fs::remove_file(&target.path)
    };

    match res {
        Ok(()) => true,
        Err(err) => {
            summary
                .failures
                .push((target.path.clone(), err.to_string()));
            false
        }
    }
}

/// Fault-tolerant deletion loop executing on selected targets using system privilege status.
fn execute_deletions(targets: &[&CleanTarget]) -> DeletionSummary {
    execute_deletions_with(targets, is_elevated())
}

/// Internal fault-tolerant deletion loop parameterised by elevation status for deterministic testing.
pub(crate) fn execute_deletions_with(targets: &[&CleanTarget], elevated: bool) -> DeletionSummary {
    let mut summary = DeletionSummary::default();

    for target in targets {
        if target.requires_elevation && !elevated {
            summary.skipped_privilege.push((*target).clone());
            continue;
        }

        let mut target_deleted = false;
        let mut target_bytes = 0;

        if target.is_tree {
            if delete_tree_target(target, &mut summary) {
                target_deleted = true;
                target_bytes = target.size_bytes;
            }
        } else if !target.files.is_empty() {
            let reclaimed = delete_files_target(target, &mut summary);
            if reclaimed > 0 {
                target_deleted = true;
                target_bytes = reclaimed;
            }
        } else if delete_single_path_target(target, &mut summary) {
            target_deleted = true;
            target_bytes = target.size_bytes;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_execute_deletions_tree_success() {
        let dir = tempdir().expect("tempdir");
        let tree_path = dir.path().join("artifacts_tree");
        fs::create_dir_all(&tree_path).expect("create dir");
        let f1 = tree_path.join("file1.bin");
        File::create(&f1)
            .expect("create file")
            .write_all(&[1u8; 100])
            .expect("write");

        let target = CleanTarget::new(
            "Test Tree".to_string(),
            tree_path.clone(),
            CleanCategory::Artifacts,
            100,
            1,
            false,
            true,
            Vec::new(),
            "test tree deletion".to_string(),
        );

        let summary = execute_deletions_with(&[&target], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 100);
        assert!(summary.failures.is_empty());
        assert!(summary.skipped_privilege.is_empty());
        assert!(!tree_path.exists());
    }

    #[test]
    fn test_execute_deletions_single_file_success() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("single.log");
        File::create(&file_path)
            .expect("create file")
            .write_all(&[2u8; 50])
            .expect("write");

        let target = CleanTarget::new(
            "Test Log File".to_string(),
            file_path.clone(),
            CleanCategory::LogsCache,
            50,
            1,
            false,
            false,
            Vec::new(),
            "test single file deletion".to_string(),
        );

        let summary = execute_deletions_with(&[&target], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 50);
        assert!(summary.failures.is_empty());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_execute_deletions_multi_file_partial() {
        let dir = tempdir().expect("tempdir");
        let f1 = dir.path().join("f1.tmp");
        let f2 = dir.path().join("f2.tmp");
        let f3_missing = dir.path().join("f3_missing.tmp");

        File::create(&f1)
            .expect("create f1")
            .write_all(&[0u8; 120])
            .expect("write f1");
        File::create(&f2)
            .expect("create f2")
            .write_all(&[0u8; 80])
            .expect("write f2");

        let target = CleanTarget::new(
            "Multi-File Target".to_string(),
            dir.path().to_path_buf(),
            CleanCategory::LogsCache,
            200,
            3,
            false,
            false,
            vec![f1.clone(), f3_missing.clone(), f2.clone()],
            "test multi file partial deletion".to_string(),
        );

        let summary = execute_deletions_with(&[&target], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 200);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].0, f3_missing);
        assert!(!f1.exists());
        assert!(!f2.exists());
    }

    #[test]
    fn test_execute_deletions_fault_tolerant_mixed() {
        let dir = tempdir().expect("tempdir");
        let ok_dir = dir.path().join("ok_dir");
        fs::create_dir_all(&ok_dir).expect("create ok_dir");
        let missing_path = dir.path().join("does_not_exist");
        let ok_file = dir.path().join("ok_file.txt");
        File::create(&ok_file)
            .expect("create ok_file")
            .write_all(&[0u8; 40])
            .expect("write");

        let t_ok1 = CleanTarget::new(
            "Valid Tree".to_string(),
            ok_dir.clone(),
            CleanCategory::Artifacts,
            500,
            1,
            false,
            true,
            Vec::new(),
            "reason".to_string(),
        );
        let t_fail = CleanTarget::new(
            "Missing Tree".to_string(),
            missing_path.clone(),
            CleanCategory::Artifacts,
            300,
            1,
            false,
            true,
            Vec::new(),
            "reason".to_string(),
        );
        let t_ok2 = CleanTarget::new(
            "Valid File".to_string(),
            ok_file.clone(),
            CleanCategory::LogsCache,
            40,
            1,
            false,
            false,
            Vec::new(),
            "reason".to_string(),
        );

        let summary = execute_deletions_with(&[&t_ok1, &t_fail, &t_ok2], false);
        assert_eq!(summary.deleted_targets, 2);
        assert_eq!(summary.reclaimed_bytes, 540);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].0, missing_path);
        assert!(!ok_dir.exists());
        assert!(!ok_file.exists());
    }

    #[test]
    fn test_execute_deletions_root_skipping() {
        let dir = tempdir().expect("tempdir");
        let root_dir = dir.path().join("root_owned");
        fs::create_dir_all(&root_dir).expect("create root_dir");
        let user_dir = dir.path().join("user_owned");
        fs::create_dir_all(&user_dir).expect("create user_dir");

        let t_root = CleanTarget::new(
            "Root Target".to_string(),
            root_dir.clone(),
            CleanCategory::PackageCache,
            1000,
            1,
            true,
            true,
            Vec::new(),
            "root required".to_string(),
        );
        let t_user = CleanTarget::new(
            "User Target".to_string(),
            user_dir.clone(),
            CleanCategory::Artifacts,
            200,
            1,
            false,
            true,
            Vec::new(),
            "user target".to_string(),
        );

        // Unprivileged: skips root target
        let unpriv_summary = execute_deletions_with(&[&t_root, &t_user], false);
        assert_eq!(unpriv_summary.skipped_privilege.len(), 1);
        assert_eq!(unpriv_summary.skipped_privilege[0].title, "Root Target");
        assert_eq!(unpriv_summary.deleted_targets, 1);
        assert_eq!(unpriv_summary.reclaimed_bytes, 200);
        assert!(root_dir.exists());
        assert!(!user_dir.exists());

        // Privileged: deletes root target
        let priv_summary = execute_deletions_with(&[&t_root], true);
        assert!(priv_summary.skipped_privilege.is_empty());
        assert_eq!(priv_summary.deleted_targets, 1);
        assert_eq!(priv_summary.reclaimed_bytes, 1000);
        assert!(!root_dir.exists());
    }
}
