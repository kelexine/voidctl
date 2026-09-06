// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-06
// Purpose: Search options, entry type classification, case sensitivity modes, and pattern normalization

use globset::{GlobBuilder, GlobMatcher};
use std::path::PathBuf;

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

/// Case sensitivity mode for pattern matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaseMode {
    /// Case-insensitive if pattern contains only lowercase; case-sensitive if it contains any uppercase.
    #[default]
    Smart,
    /// Force case-sensitive matching.
    Sensitive,
    /// Force case-insensitive matching.
    Insensitive,
}

/// Checks whether a pattern contains glob metacharacters (`*`, `?`, `[`).
#[must_use]
pub fn has_glob_metachars(pattern: &str) -> bool {
    pattern.chars().any(|c| matches!(c, '*' | '?' | '['))
}

/// Normalizes a search pattern.
///
/// If `exact` is false and the pattern contains no glob metacharacters,
/// it automatically wraps the pattern in `*<pattern>*` for intuitive substring matching.
#[must_use]
pub fn normalize_pattern(pattern: &str, exact: bool) -> String {
    if exact || has_glob_metachars(pattern) {
        pattern.to_string()
    } else {
        format!("*{pattern}*")
    }
}

/// Options controlling how a search is executed.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Pattern passed by user or caller.
    pub pattern: String,
    /// Entry type filter.
    pub entry_type: SearchType,
    /// Root directory to search from.
    pub root: PathBuf,
    /// Case sensitivity mode.
    pub case_mode: CaseMode,
    /// Require exact name match (disables automatic substring wrapping).
    pub exact: bool,
    /// Maximum number of results to display (0 = unlimited).
    pub limit: usize,
    /// Whether to follow symlinks.
    pub follow_links: bool,
    /// Whether to search hidden files and directories.
    pub hidden: bool,
}

impl SearchOptions {
    /// Computes the effective glob pattern to compile.
    #[must_use]
    pub fn effective_pattern(&self) -> String {
        normalize_pattern(&self.pattern, self.exact)
    }

    /// Determines whether matching should be case-insensitive based on [`CaseMode`].
    #[must_use]
    pub fn is_case_insensitive(&self) -> bool {
        match self.case_mode {
            CaseMode::Insensitive => true,
            CaseMode::Sensitive => false,
            CaseMode::Smart => !self.pattern.chars().any(|c| c.is_uppercase()),
        }
    }

    /// Build a [`GlobMatcher`] from the effective pattern and case mode.
    pub fn build_matcher(&self) -> anyhow::Result<GlobMatcher> {
        let pattern = self.effective_pattern();
        let case_insensitive = self.is_case_insensitive();

        let matcher = GlobBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .literal_separator(false)
            .build()
            .map_err(|e| anyhow::anyhow!("Invalid glob pattern '{}': {}", pattern, e))?
            .compile_matcher();
        Ok(matcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_pattern() {
        assert_eq!(normalize_pattern("siwes", false), "*siwes*");
        assert_eq!(normalize_pattern("siwes", true), "siwes");
        assert_eq!(normalize_pattern("*.rs", false), "*.rs");
        assert_eq!(normalize_pattern("*siwes*", false), "*siwes*");
        assert_eq!(normalize_pattern("test_?", false), "test_?");
        assert_eq!(normalize_pattern("[a-z]*", false), "[a-z]*");
    }

    #[test]
    fn test_search_type_from_str() {
        assert_eq!("file".parse::<SearchType>().ok(), Some(SearchType::File));
        assert_eq!("f".parse::<SearchType>().ok(), Some(SearchType::File));
        assert_eq!("d".parse::<SearchType>().ok(), Some(SearchType::Dir));
        assert_eq!(
            "directory".parse::<SearchType>().ok(),
            Some(SearchType::Dir)
        );
        assert_eq!("any".parse::<SearchType>().ok(), Some(SearchType::Any));
        assert!("bogus".parse::<SearchType>().is_err());
    }
}
