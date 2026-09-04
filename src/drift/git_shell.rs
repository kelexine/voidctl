// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Thin wrapper shelling out to git status --porcelain for dotfiles drift

use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Error conditions when querying git status.
#[derive(Debug, Error)]
pub enum GitDriftError {
    #[error("Dotfiles directory '{0}' does not exist")]
    DirectoryNotFound(String),
    #[error("Failed to execute git command: {0}")]
    GitExecutionFailed(#[from] std::io::Error),
    #[error("git status failed with exit code: {0}")]
    GitExitError(i32),
}

/// A parsed entry from `git status --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusEntry {
    pub index_state: char,
    pub worktree_state: char,
    pub path: String,
}

/// Runs `git -C <dotfiles_dir> status --porcelain` and parses output lines.
pub fn query_repo_status(dotfiles_dir: &Path) -> Result<Vec<GitStatusEntry>, GitDriftError> {
    if !dotfiles_dir.exists() {
        return Err(GitDriftError::DirectoryNotFound(
            dotfiles_dir.display().to_string(),
        ));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(dotfiles_dir)
        .arg("status")
        .arg("--porcelain")
        .output()?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(GitDriftError::GitExitError(code));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_porcelain_output(&stdout))
}

/// Parses porcelain lines into structured entries.
#[must_use]
pub fn parse_porcelain_output(stdout: &str) -> Vec<GitStatusEntry> {
    let mut entries = Vec::new();

    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }

        let mut chars = line.chars();
        let index_state = chars.next().unwrap_or(' ');
        let worktree_state = chars.next().unwrap_or(' ');
        let _space = chars.next();
        let raw_path: String = chars.collect();
        let trimmed = raw_path.trim();
        // In git porcelain format, renames appear as "R  old-path -> new-path"
        let path = if let Some((_old, new)) = trimmed.split_once(" -> ") {
            new.trim().to_string()
        } else {
            trimmed.to_string()
        };

        entries.push(GitStatusEntry {
            index_state,
            worktree_state,
            path,
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_porcelain_lines() {
        let output = " M bash/.bashrc\nM  zsh/.zshrc\n?? newfile.sh\nR  old.sh -> renamed.sh\n";
        let entries = parse_porcelain_output(output);

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].index_state, ' ');
        assert_eq!(entries[0].worktree_state, 'M');
        assert_eq!(entries[0].path, "bash/.bashrc");

        assert_eq!(entries[1].index_state, 'M');
        assert_eq!(entries[1].worktree_state, ' ');
        assert_eq!(entries[1].path, "zsh/.zshrc");

        assert_eq!(entries[2].index_state, '?');
        assert_eq!(entries[2].worktree_state, '?');
        assert_eq!(entries[2].path, "newfile.sh");

        assert_eq!(entries[3].index_state, 'R');
        assert_eq!(entries[3].worktree_state, ' ');
        assert_eq!(entries[3].path, "renamed.sh");
    }
}
