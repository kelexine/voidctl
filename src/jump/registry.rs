// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Alias registry lookup, registration, and listing for voidctl jump

use crate::config::Config;
use std::path::{Path, PathBuf};

/// Resolves an alias to its target directory path.
#[must_use]
pub fn resolve_alias<'a>(config: &'a Config, alias: &str) -> Option<&'a Path> {
    config.jump.aliases.get(alias).map(PathBuf::as_path)
}

/// Registers or updates a jump alias pointing to the specified path.
pub fn add_alias(config: &mut Config, alias: String, path: PathBuf) {
    config.jump.aliases.insert(alias, path);
}

/// Returns a sorted list of all registered aliases and their corresponding paths.
#[must_use]
pub fn list_aliases(config: &Config) -> Vec<(&str, &Path)> {
    let mut list: Vec<(&str, &Path)> = config
        .jump
        .aliases
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_path()))
        .collect();
    list.sort_by_key(|(k, _)| *k);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jump_registry() {
        let mut config = Config::default();
        add_alias(
            &mut config,
            "void".to_string(),
            PathBuf::from("/home/kelexine/dev/voidctl"),
        );

        assert_eq!(
            resolve_alias(&config, "void"),
            Some(Path::new("/home/kelexine/dev/voidctl"))
        );
        assert_eq!(resolve_alias(&config, "nonexistent"), None);

        let list = list_aliases(&config);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "void");
    }
}
