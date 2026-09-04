// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Interactive multi-selection and bulk category selection for clean operations

use crate::clean::classifier::{CleanCategory, CleanItem};
use crate::clean::walker::CleanReport;
use humansize::{DECIMAL, format_size};
use inquire::{Confirm, MultiSelect};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

/// Error conditions during interactive selection or deletion.
#[derive(Debug, Error)]
pub enum CleanError {
    #[error("Interactive prompt aborted: {0}")]
    PromptAborted(String),
    #[error("Failed to delete '{path}': {source}")]
    DeleteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Helper display wrapper for category bulk toggle choices.
#[derive(Clone, PartialEq, Eq)]
struct CategoryOption {
    category: CleanCategory,
    count: usize,
    size_bytes: u64,
}

impl fmt::Display for CategoryOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size_str = format_size(self.size_bytes, DECIMAL);
        write!(
            f,
            "All {} ({} items, {})",
            self.category, self.count, size_str
        )
    }
}

/// Helper display wrapper for individual items in multi-select prompt.
#[derive(Clone, PartialEq, Eq)]
struct ItemOption {
    index: usize,
    category: CleanCategory,
    path: PathBuf,
    size_bytes: u64,
}

impl fmt::Display for ItemOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size_str = format_size(self.size_bytes, DECIMAL);
        write!(
            f,
            "[{}] {} ({})",
            self.category,
            self.path.display(),
            size_str
        )
    }
}

/// Executes interactive category-level and item-level selection, then deletes selected.
pub fn interactive_select_and_clean(report: &CleanReport) -> Result<(usize, u64), CleanError> {
    if report.items.is_empty() {
        println!("No cleanable items found.");
        return Ok((0, 0));
    }

    let preselected_indices = prompt_category_bulk_selection(&report.items)?;
    let chosen_items = prompt_item_selection(&report.items, &preselected_indices)?;

    if chosen_items.is_empty() {
        println!("No items selected. Exiting without changes.");
        return Ok((0, 0));
    }

    let total_size: u64 = chosen_items.iter().map(|i| i.size_bytes).sum();
    let confirmed = prompt_confirmation(chosen_items.len(), total_size)?;

    if !confirmed {
        println!("Cleanup aborted by user.");
        return Ok((0, 0));
    }

    execute_deletions(&chosen_items)
}

/// Prompts user to select categories for bulk inclusion.
fn prompt_category_bulk_selection(items: &[CleanItem]) -> Result<HashSet<usize>, CleanError> {
    let options = build_category_options(items);
    if options.is_empty() {
        return Ok(HashSet::new());
    }

    let prompt = "Select categories to bulk-toggle (or press Enter to customize individually):";
    let selected = MultiSelect::new(prompt, options)
        .prompt()
        .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

    let chosen_categories: HashSet<CleanCategory> =
        selected.into_iter().map(|opt| opt.category).collect();

    let preselected: HashSet<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if chosen_categories.contains(&item.category) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    Ok(preselected)
}

/// Aggregates category counts and sizes for the bulk prompt.
fn build_category_options(items: &[CleanItem]) -> Vec<CategoryOption> {
    let mut map: std::collections::HashMap<CleanCategory, (usize, u64)> =
        std::collections::HashMap::new();

    for item in items {
        let entry = map.entry(item.category).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += item.size_bytes;
    }

    let mut options: Vec<CategoryOption> = map
        .into_iter()
        .map(|(category, (count, size_bytes))| CategoryOption {
            category,
            count,
            size_bytes,
        })
        .collect();
    options.sort_by_key(|opt| opt.category);
    options
}

/// Prompts user with the full item list, pre-selecting bulk items.
fn prompt_item_selection<'a>(
    items: &'a [CleanItem],
    preselected: &HashSet<usize>,
) -> Result<Vec<&'a CleanItem>, CleanError> {
    let options: Vec<ItemOption> = items
        .iter()
        .enumerate()
        .map(|(index, item)| ItemOption {
            index,
            category: item.category,
            path: item.path.clone(),
            size_bytes: item.size_bytes,
        })
        .collect();

    let default_indices: Vec<usize> = preselected.iter().copied().collect();

    let prompt = "Confirm items to delete (Space to toggle, Enter to confirm):";
    let chosen_options = MultiSelect::new(prompt, options)
        .with_default(&default_indices)
        .prompt()
        .map_err(|e| CleanError::PromptAborted(e.to_string()))?;

    Ok(chosen_options
        .into_iter()
        .map(|opt| &items[opt.index])
        .collect())
}

/// Confirms the deletion action with the user.
fn prompt_confirmation(count: usize, total_size: u64) -> Result<bool, CleanError> {
    let size_str = format_size(total_size, DECIMAL);
    let message = format!("Permanently delete {count} selected targets (reclaiming {size_str})?");
    Confirm::new(&message)
        .with_default(false)
        .prompt()
        .map_err(|e| CleanError::PromptAborted(e.to_string()))
}

/// Deletes each selected item and returns total deleted count and reclaimed bytes.
fn execute_deletions(items: &[&CleanItem]) -> Result<(usize, u64), CleanError> {
    let mut deleted_count = 0;
    let mut reclaimed_bytes: u64 = 0;

    for item in items {
        if item.is_dir {
            if item.path.exists() {
                fs::remove_dir_all(&item.path).map_err(|source| CleanError::DeleteFailed {
                    path: item.path.clone(),
                    source,
                })?;
            }
        } else if item.path.exists() {
            fs::remove_file(&item.path).map_err(|source| CleanError::DeleteFailed {
                path: item.path.clone(),
                source,
            })?;
        }
        deleted_count += 1;
        reclaimed_bytes += item.size_bytes;
    }

    Ok((deleted_count, reclaimed_bytes))
}
