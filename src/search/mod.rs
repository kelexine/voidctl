// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Pattern-based filesystem search within a configurable root directory

use colored::Colorize;
use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Which entry types to include in search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchType {
    File,
    Dir,
    Any,
}

impl std::str::FromStr for SearchType {
    type Err = String;

    fn from_str(s: &str) -> Result<SearchType, String> {
        match s.trim().to_lowercase().as_str() {
            "f" | "file" => Ok(SearchType::File),
            "d" | "dir" | "directory" => Ok(SearchType::Dir),
            "a" | "any" | "all" => Ok(SearchType::Any),
            other => Err(format!(
                "Unknown type '{}'. Valid values: file (f), dir (d), any (a)",
                other
            )),
        }
    }
}

/// Options controlling how a search is executed.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Glob pattern to match against entry names (e.g. `*.log`, `config*`).
    pub pattern: String,
    /// Entry type filter.
    pub entry_type: SearchType,
    /// Root directory to search from.
    pub root: PathBuf,
    /// Whether the match is case-insensitive.
    pub case_insensitive: bool,
    /// Maximum number of results to display (0 = unlimited).
    pub limit: usize,
    /// Whether to follow symlinks.
    pub follow_links: bool,
    /// Whether to search hidden files and directories.
    pub hidden: bool,
}

impl SearchOptions {
    /// Build a [`GlobMatcher`] from the pattern in these options.
    pub fn build_matcher(&self) -> anyhow::Result<GlobMatcher> {
        let matcher = GlobBuilder::new(&self.pattern)
            .case_insensitive(self.case_insensitive)
            .literal_separator(false)
            .build()
            .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", self.pattern, e))?
            .compile_matcher();
        Ok(matcher)
    }
}

/// A single search hit.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Run the filesystem search and print results to stdout.
///
/// Returns the number of matches found, or an error if the pattern is invalid.
pub fn execute_search(opts: &SearchOptions) -> anyhow::Result<usize> {
    if !opts.root.exists() {
        anyhow::bail!("Search root does not exist: {}", opts.root.display());
    }

    let matcher = opts.build_matcher()?;

    let walker = WalkBuilder::new(&opts.root)
        .follow_links(opts.follow_links)
        .hidden(!opts.hidden)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .build();

    let limit = if opts.limit == 0 { usize::MAX } else { opts.limit };
    let mut hits: Vec<SearchHit> = Vec::new();

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Never emit the root itself.
        if entry.path() == opts.root {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        // Type filter.
        let type_matches = match opts.entry_type {
            SearchType::File => !is_dir,
            SearchType::Dir => is_dir,
            SearchType::Any => true,
        };
        if !type_matches {
            continue;
        }

        // Pattern match against the file/dir name only.
        let name = entry.file_name().to_string_lossy();
        if !matcher.is_match(name.as_ref()) {
            continue;
        }

        hits.push(SearchHit {
            path: entry.path().to_owned(),
            is_dir,
        });

        if hits.len() >= limit {
            break;
        }
    }

    print_search_results(&hits, opts, limit);
    Ok(hits.len())
}

/// Render search results with colored output.
fn print_search_results(hits: &[SearchHit], opts: &SearchOptions, limit: usize) {
    let root_display = opts.root.display().to_string();

    if hits.is_empty() {
        println!(
            "{}",
            format!(
                "No matches for '{}' under {}",
                opts.pattern, root_display
            )
            .yellow()
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
fn prettify_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| format!("./{}", rel.display()))
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_tree() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("sub/deep")).unwrap();
        fs::write(root.join("foo.log"), b"log content").unwrap();
        fs::write(root.join("bar.txt"), b"text content").unwrap();
        fs::write(root.join("sub/baz.log"), b"sub log").unwrap();
        fs::write(root.join("sub/deep/qux.rs"), b"rust code").unwrap();
        dir
    }

    fn opts(pattern: &str, root: &std::path::Path) -> SearchOptions {
        SearchOptions {
            pattern: pattern.to_string(),
            entry_type: SearchType::Any,
            root: root.to_path_buf(),
            case_insensitive: false,
            limit: 0,
            follow_links: false,
            hidden: false,
        }
    }

    #[test]
    fn test_search_glob_log_files() {
        let dir = make_tree();
        let mut o = opts("*.log", dir.path());
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 2, "Expected 2 .log files");
    }

    #[test]
    fn test_search_type_dir_only() {
        let dir = make_tree();
        let mut o = opts("*", dir.path());
        o.entry_type = SearchType::Dir;
        let n = execute_search(&o).unwrap();
        // sub/ and sub/deep/
        assert!(n >= 2, "Expected at least 2 directories");
    }

    #[test]
    fn test_search_limit() {
        let dir = make_tree();
        let mut o = opts("*", dir.path());
        o.limit = 2;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 2, "Limit should cap results at 2");
    }

    #[test]
    fn test_search_case_insensitive() {
        let dir = make_tree();
        let mut o = opts("*.LOG", dir.path());
        o.case_insensitive = true;
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 2, "Case-insensitive *.LOG should match *.log files");
    }

    #[test]
    fn test_search_no_matches() {
        let dir = make_tree();
        let o = opts("*.xyz", dir.path());
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_search_invalid_root() {
        let o = SearchOptions {
            pattern: "*".to_string(),
            entry_type: SearchType::Any,
            root: PathBuf::from("/does/not/exist/ever"),
            case_insensitive: false,
            limit: 0,
            follow_links: false,
            hidden: false,
        };
        assert!(execute_search(&o).is_err());
    }

    #[test]
    fn test_search_type_from_str() {
        assert_eq!("file".parse::<SearchType>().ok(), Some(SearchType::File));
        assert_eq!("f".parse::<SearchType>().ok(), Some(SearchType::File));
        assert_eq!("d".parse::<SearchType>().ok(), Some(SearchType::Dir));
        assert_eq!("directory".parse::<SearchType>().ok(), Some(SearchType::Dir));
        assert_eq!("any".parse::<SearchType>().ok(), Some(SearchType::Any));
        assert!("bogus".parse::<SearchType>().is_err());
    }
}
