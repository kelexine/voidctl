// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Drift auditing subsystem for dotfiles repository and symlink tree

pub mod git_shell;
pub mod links;

pub use git_shell::{GitDriftError, GitStatusEntry, query_repo_status};
pub use links::{LinkRecord, LinkStatus, evaluate_link, verify_symlinks};

use std::collections::HashMap;
use std::path::Path;

/// Aggregate report of dotfiles repository state and mapped symlinks.
#[derive(Debug, Clone)]
pub struct DriftAuditReport {
    pub link_records: Vec<LinkRecord>,
    pub git_entries: Result<Vec<GitStatusEntry>, String>,
}

/// Runs a complete audit across configured symlinks and git repository status.
#[must_use]
pub fn audit_drift(
    dotfiles_dir: &Path,
    home_dir: &Path,
    links: &HashMap<String, String>,
) -> DriftAuditReport {
    let link_records = verify_symlinks(dotfiles_dir, home_dir, links);
    let git_entries = query_repo_status(dotfiles_dir).map_err(|e| e.to_string());

    DriftAuditReport {
        link_records,
        git_entries,
    }
}
