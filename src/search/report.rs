// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Formatted terminal rendering and path presentation for search results

use super::SearchHit;
use super::options::SearchOptions;
use colored::Colorize;
use std::path::Path;

/// Render search results with colored output.
pub fn print_search_results(hits: &[SearchHit], opts: &SearchOptions, limit: usize) {
    let root_display = opts.root.display().to_string();

    if hits.is_empty() {
        println!(
            "{}",
            format!("No matches for '{}' under {}", opts.pattern, root_display).yellow()
        );
        return;
    }

    println!(
        "{:6}  {:<8}  {}",
        "#".bold().cyan(),
        "TYPE".bold().cyan(),
        "PATH".bold().cyan()
    );
    println!("{}", "-".repeat(80).dimmed());

    for (i, hit) in hits.iter().enumerate() {
        let idx = format!("{:>6}", i + 1).dimmed();
        let type_tag = if hit.is_dir {
            "dir ".blue().bold()
        } else {
            "file".normal()
        };
        let path_str = prettify_path(&hit.path, &opts.root);
        let path_colored = if hit.is_dir {
            path_str.blue()
        } else {
            path_str.normal()
        };
        println!("{idx}  {type_tag:<8}  {path_colored}");
    }

    println!("{}", "-".repeat(80).dimmed());
    let summary = format!(
        "{} match(es) for '{}' under {}",
        hits.len(),
        opts.pattern,
        root_display
    );
    if opts.limit != 0 && hits.len() >= limit {
        println!(
            "{}  {}",
            summary.bold(),
            format!("(limit {} reached — use --limit 0 for all)", opts.limit).yellow()
        );
    } else {
        println!("{}", summary.bold());
    }
}

/// Format path relative to root if possible, otherwise use absolute.
#[must_use]
pub fn prettify_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| format!("./{}", rel.display()))
        .unwrap_or_else(|_| path.display().to_string())
}
