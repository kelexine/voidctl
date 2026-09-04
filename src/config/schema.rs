// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Configuration schemas and Serde data structures for voidctl

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Default age threshold in days for log and cache pruning.
pub const DEFAULT_AGE_THRESHOLD_DAYS: u64 = 7;

/// Top-level configuration object stored in ~/.config/voidctl/voidctl.toml.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Config {
    /// Jump and project-command configurations.
    #[serde(default)]
    pub jump: JumpConfig,
    /// System hygiene cleanup configuration.
    #[serde(default)]
    pub clean: CleanConfig,
    /// Config drift detection settings for dotfiles.
    #[serde(default)]
    pub drift: DriftConfig,
}

/// Project jump aliases and registered commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JumpConfig {
    /// Map of alias -> project root directory path.
    pub aliases: HashMap<String, PathBuf>,
    /// Map of alias -> map of command name -> shell command string.
    pub commands: HashMap<String, HashMap<String, String>>,
}

impl Serialize for JumpConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for (alias, path) in &self.aliases {
            map.serialize_entry(alias, &path.to_string_lossy())?;
        }
        if !self.commands.is_empty() {
            map.serialize_entry("commands", &self.commands)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for JumpConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct JumpConfigVisitor;

        impl<'de> Visitor<'de> for JumpConfigVisitor {
            type Value = JumpConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a jump table with aliases and commands")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut aliases = HashMap::new();
                let mut commands = HashMap::new();

                while let Some(key) = access.next_key::<String>()? {
                    if key == "commands" {
                        commands =
                            access.next_value::<HashMap<String, HashMap<String, String>>>()?;
                    } else {
                        let path_str = access.next_value::<String>()?;
                        aliases.insert(key, PathBuf::from(path_str));
                    }
                }

                Ok(JumpConfig { aliases, commands })
            }
        }

        deserializer.deserialize_map(JumpConfigVisitor)
    }
}

/// System hygiene scan roots, exclusions, and retention parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CleanConfig {
    /// Standard scan roots evaluated on machine hygiene passes.
    #[serde(default = "default_scan_roots")]
    pub scan_roots: Vec<PathBuf>,
    /// Maximum age in days before logs and cache entries are marked stale.
    #[serde(default = "default_age_threshold_days")]
    pub age_threshold_days: u64,
    /// Directory names or globs excluded from cleanup scans.
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
    /// Additional custom scan roots (e.g. dotfiles backup directories).
    #[serde(default)]
    pub extra_scan_roots: Vec<PathBuf>,
}

impl Default for CleanConfig {
    fn default() -> Self {
        Self {
            scan_roots: default_scan_roots(),
            age_threshold_days: default_age_threshold_days(),
            exclude: default_exclude(),
            extra_scan_roots: Vec::new(),
        }
    }
}

/// Dotfiles drift verification settings and mapped symlink pairs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftConfig {
    /// Absolute or resolved path to dotfiles git repository.
    #[serde(default = "default_dotfiles_dir")]
    pub dotfiles_dir: PathBuf,
    /// Map of source file (relative to dotfiles_dir) to target (relative to $HOME).
    #[serde(default = "default_symlinks")]
    pub links: HashMap<String, String>,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            dotfiles_dir: default_dotfiles_dir(),
            links: default_symlinks(),
        }
    }
}

/// Returns default system-level and user scan roots.
#[must_use]
pub fn default_scan_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/var/cache/pacman/pkg"),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
    ]
}

/// Returns default age threshold of 7 days.
#[must_use]
pub const fn default_age_threshold_days() -> u64 {
    DEFAULT_AGE_THRESHOLD_DAYS
}

/// Returns default ignore list during file walks.
#[must_use]
pub fn default_exclude() -> Vec<String> {
    vec![
        String::from(".git"),
        String::from(".cargo/registry"),
        String::from(".rustup"),
    ]
}

/// Returns default dotfiles directory located at ~/dotfiles or ~/.dotfiles.
#[must_use]
pub fn default_dotfiles_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let standard_dotfiles = PathBuf::from(&home).join("dotfiles");
        if standard_dotfiles.exists() {
            return standard_dotfiles;
        }
        PathBuf::from(home).join(".dotfiles")
    } else {
        PathBuf::from("dotfiles")
    }
}

/// Returns the baseline symlink mapping from ADR-0001.
#[must_use]
pub fn default_symlinks() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(String::from("bash/.bashrc"), String::from(".bashrc"));
    map.insert(String::from("zsh/.zshrc"), String::from(".zshrc"));
    map.insert(
        String::from("fish/config.fish"),
        String::from(".config/fish/config.fish"),
    );
    map.insert(
        String::from("fish/functions/extract.fish"),
        String::from(".config/fish/functions/extract.fish"),
    );
    map.insert(String::from("git/.gitconfig"), String::from(".gitconfig"));
    map.insert(
        String::from("git/.gitignore_global"),
        String::from(".gitignore_global"),
    );
    map.insert(String::from("ssh/config"), String::from(".ssh/config"));
    map.insert(
        String::from("ripgrep/ripgreprc"),
        String::from(".config/ripgrep/ripgreprc"),
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jump_config_roundtrip() {
        let toml_str = r#"
[jump]
findx = "/home/kelexine/projects/findx"

[jump.commands.findx]
test = "cargo test --workspace"
build = "cargo build --release"
"#;
        let config: Config = toml::from_str(toml_str).expect("failed to deserialize config");
        assert_eq!(
            config.jump.aliases.get("findx"),
            Some(&PathBuf::from("/home/kelexine/projects/findx"))
        );
        let findx_cmds = config.jump.commands.get("findx").unwrap();
        assert_eq!(findx_cmds.get("test").unwrap(), "cargo test --workspace");
        assert_eq!(findx_cmds.get("build").unwrap(), "cargo build --release");

        let serialized = toml::to_string(&config).expect("failed to serialize config");
        let roundtrip: Config =
            toml::from_str(&serialized).expect("failed to deserialize roundtrip");
        assert_eq!(config, roundtrip);
    }
}
