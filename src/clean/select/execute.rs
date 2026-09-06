// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Fault-tolerant cleanup deletion engine, privilege awareness, and deletion reporting

use crate::clean::classifier::CleanTarget;
use crate::clean::privilege::is_elevated;
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use std::fs;
use std::path::PathBuf;

/// Result summary of clean execution.
#[derive(Debug, Default)]
pub struct DeletionSummary {
    pub deleted_targets: usize,
    pub reclaimed_bytes: u64,
    pub skipped_privilege: Vec<CleanTarget>,
    pub failures: Vec<(PathBuf, String)>,
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
#[must_use]
pub fn execute_deletions(targets: &[&CleanTarget]) -> DeletionSummary {
    execute_deletions_with(targets, is_elevated())
}

/// Internal fault-tolerant deletion loop parameterised by elevation status for deterministic testing.
#[must_use]
pub fn execute_deletions_with(targets: &[&CleanTarget], elevated: bool) -> DeletionSummary {
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
pub fn print_deletion_summary(summary: &DeletionSummary) {
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
    use crate::clean::classifier::CleanCategory;
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
            "Single File".to_string(),
            file_path.clone(),
            CleanCategory::LogsCache,
            50,
            1,
            false,
            false,
            Vec::new(),
            "test file deletion".to_string(),
        );

        let summary = execute_deletions_with(&[&target], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 50);
        assert!(summary.failures.is_empty());
        assert!(summary.skipped_privilege.is_empty());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_execute_deletions_multi_file_partial() {
        let dir = tempdir().expect("tempdir");
        let base_dir = dir.path().join("multi");
        fs::create_dir_all(&base_dir).expect("create dir");

        let f1 = base_dir.join("f1.tmp");
        let f2 = base_dir.join("f2.tmp");
        File::create(&f1)
            .expect("create f1")
            .write_all(&[3u8; 40])
            .expect("write");
        File::create(&f2)
            .expect("create f2")
            .write_all(&[4u8; 60])
            .expect("write");

        let non_existent = base_dir.join("ghost.tmp");

        let target = CleanTarget::new(
            "Loose Files".to_string(),
            base_dir.clone(),
            CleanCategory::LogsCache,
            100,
            3,
            false,
            false,
            vec![f1.clone(), f2.clone(), non_existent],
            "test loose files deletion".to_string(),
        );

        let summary = execute_deletions_with(&[&target], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 100);
        assert_eq!(summary.failures.len(), 1);
        assert!(!f1.exists());
        assert!(!f2.exists());
    }

    #[test]
    fn test_execute_deletions_fault_tolerant_mixed() {
        let dir = tempdir().expect("tempdir");
        let valid_file = dir.path().join("valid.cache");
        File::create(&valid_file)
            .expect("create file")
            .write_all(&[5u8; 80])
            .expect("write");

        let non_existent_tree = dir.path().join("no_such_tree");

        let t1 = CleanTarget::new(
            "Valid File".to_string(),
            valid_file.clone(),
            CleanCategory::LogsCache,
            80,
            1,
            false,
            false,
            Vec::new(),
            "valid file".to_string(),
        );

        let t2 = CleanTarget::new(
            "Missing Tree".to_string(),
            non_existent_tree.clone(),
            CleanCategory::Artifacts,
            200,
            1,
            false,
            true,
            Vec::new(),
            "missing tree".to_string(),
        );

        let summary = execute_deletions_with(&[&t1, &t2], false);
        assert_eq!(summary.deleted_targets, 1);
        assert_eq!(summary.reclaimed_bytes, 80);
        assert_eq!(summary.failures.len(), 1);
        assert!(!valid_file.exists());
    }

    #[test]
    fn test_execute_deletions_root_skipping() {
        let dir = tempdir().expect("tempdir");
        let sys_file = dir.path().join("system.pkg");
        File::create(&sys_file)
            .expect("create file")
            .write_all(&[6u8; 500])
            .expect("write");

        let target = CleanTarget::new(
            "Root Package".to_string(),
            sys_file.clone(),
            CleanCategory::PackageCache,
            500,
            1,
            true,
            false,
            Vec::new(),
            "requires root".to_string(),
        );

        // When unprivileged (elevated = false), target must be skipped
        let summary_unprivileged = execute_deletions_with(&[&target], false);
        assert_eq!(summary_unprivileged.deleted_targets, 0);
        assert_eq!(summary_unprivileged.reclaimed_bytes, 0);
        assert_eq!(summary_unprivileged.skipped_privilege.len(), 1);
        assert!(sys_file.exists());

        // When privileged (elevated = true), target must be deleted
        let summary_privileged = execute_deletions_with(&[&target], true);
        assert_eq!(summary_privileged.deleted_targets, 1);
        assert_eq!(summary_privileged.reclaimed_bytes, 500);
        assert!(summary_privileged.skipped_privilege.is_empty());
        assert!(!sys_file.exists());
    }
}
