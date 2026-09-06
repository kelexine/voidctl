// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Pattern-based filesystem search with smart-casing, auto-substring matching, and type filtering

pub mod options;
pub mod report;

pub use options::{CaseMode, SearchOptions, SearchType, has_glob_metachars, normalize_pattern};
pub use report::print_search_results;

use ignore::WalkBuilder;
use std::path::PathBuf;

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

    let limit = if opts.limit == 0 {
        usize::MAX
    } else {
        opts.limit
    };
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
        fs::write(root.join("sub/Siwes Logbook.md"), b"siwes doc").unwrap();
        dir
    }

    fn default_opts(pattern: &str, root: &std::path::Path) -> SearchOptions {
        SearchOptions {
            pattern: pattern.to_string(),
            entry_type: SearchType::Any,
            root: root.to_path_buf(),
            case_mode: CaseMode::Smart,
            exact: false,
            limit: 0,
            follow_links: false,
            hidden: false,
        }
    }

    #[test]
    fn test_search_glob_log_files() {
        let dir = make_tree();
        let mut o = default_opts("*.log", dir.path());
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 2, "Expected 2 .log files");
    }

    #[test]
    fn test_search_type_dir_only() {
        let dir = make_tree();
        let mut o = default_opts("*", dir.path());
        o.entry_type = SearchType::Dir;
        let n = execute_search(&o).unwrap();
        assert!(n >= 2, "Expected at least 2 directories");
    }

    #[test]
    fn test_search_limit() {
        let dir = make_tree();
        let mut o = default_opts("*", dir.path());
        o.limit = 2;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 2, "Limit should cap results at 2");
    }

    #[test]
    fn test_search_smart_case_lowercase_matches_uppercase() {
        let dir = make_tree();
        let mut o = default_opts("siwes", dir.path());
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 1, "Smart-case 'siwes' should match 'Siwes Logbook.md'");
    }

    #[test]
    fn test_search_smart_case_uppercase_enforces_sensitivity() {
        let dir = make_tree();
        let mut o = default_opts("FOO", dir.path());
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 0, "Smart-case 'FOO' must not match lowercase 'foo.log'");

        let mut o2 = default_opts("foo", dir.path());
        o2.entry_type = SearchType::File;
        let n2 = execute_search(&o2).unwrap();
        assert_eq!(n2, 1);
    }

    #[test]
    fn test_search_auto_substring_expansion() {
        let dir = make_tree();
        let mut o = default_opts("baz", dir.path());
        o.entry_type = SearchType::File;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 1, "Auto-substring 'baz' should match 'baz.log'");
    }

    #[test]
    fn test_search_exact_flag_disables_substring() {
        let dir = make_tree();
        let mut o = default_opts("baz", dir.path());
        o.exact = true;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 0, "Exact 'baz' should not match 'baz.log'");

        let mut o2 = default_opts("bar.txt", dir.path());
        o2.exact = true;
        let n2 = execute_search(&o2).unwrap();
        assert_eq!(n2, 1, "Exact 'bar.txt' should match 'bar.txt'");
    }

    #[test]
    fn test_search_explicit_case_modes() {
        let dir = make_tree();
        let mut o = default_opts("FOO", dir.path());
        o.case_mode = CaseMode::Insensitive;
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 1, "CaseMode::Insensitive should match foo.log for 'FOO'");

        let mut o2 = default_opts("siwes", dir.path());
        o2.case_mode = CaseMode::Sensitive;
        let n2 = execute_search(&o2).unwrap();
        assert_eq!(
            n2, 0,
            "CaseMode::Sensitive should NOT match Siwes for lowercase 'siwes'"
        );
    }

    #[test]
    fn test_search_no_matches() {
        let dir = make_tree();
        let o = default_opts("*.xyz", dir.path());
        let n = execute_search(&o).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_search_invalid_root() {
        let o = SearchOptions {
            pattern: "*".to_string(),
            entry_type: SearchType::Any,
            root: PathBuf::from("/does/not/exist/ever"),
            case_mode: CaseMode::Smart,
            exact: false,
            limit: 0,
            follow_links: false,
            hidden: false,
        };
        assert!(execute_search(&o).is_err());
    }
}
