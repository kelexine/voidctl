// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Configuration loading, saving, and path resolution for voidctl

pub mod schema;

pub use schema::{
    CleanConfig, Config, DEFAULT_AGE_THRESHOLD_DAYS, DriftConfig, JumpConfig,
    default_age_threshold_days, default_dotfiles_dir, default_exclude, default_scan_roots,
    default_symlinks,
};

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Configuration-related errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not determine user home directory")]
    NoHomeDir,
    #[error("Failed to read configuration file at '{path}': {source}")]
    ReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse configuration file at '{path}': {source}")]
    ParseFailed {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Failed to serialize configuration: {source}")]
    SerializeFailed {
        #[source]
        source: toml::ser::Error,
    },
    #[error("Failed to create configuration directory at '{path}': {source}")]
    DirCreateFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to write configuration file at '{path}': {source}")]
    WriteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Returns the effective user home directory, prioritizing SUDO_USER if invoked under sudo.
#[must_use]
pub fn resolve_home_dir() -> Option<PathBuf> {
    if let Ok(sudo_user) = std::env::var("SUDO_USER")
        && !sudo_user.is_empty()
        && sudo_user != "root"
    {
        let candidate = PathBuf::from(format!("/home/{sudo_user}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Resolves the path to the voidctl configuration file.
///
/// Priority:
/// 1. `VOIDCTL_CONFIG` environment variable
/// 2. `~/.config/voidctl/voidctl.toml`
pub fn resolve_config_path() -> Result<PathBuf, ConfigError> {
    if let Ok(env_path) = std::env::var("VOIDCTL_CONFIG") {
        return Ok(PathBuf::from(env_path));
    }

    let home = resolve_home_dir().ok_or(ConfigError::NoHomeDir)?;
    Ok(home.join(".config").join("voidctl").join("voidctl.toml"))
}

/// Loads the configuration from the resolved path or returns default config.
pub fn load_config() -> Result<Config, ConfigError> {
    let path = resolve_config_path()?;
    load_from_path(&path)
}

/// Loads the configuration from an explicit file path.
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(path).map_err(|source| ConfigError::ReadFailed {
        path: path.to_path_buf(),
        source,
    })?;

    toml::from_str(&contents).map_err(|source| ConfigError::ParseFailed {
        path: path.to_path_buf(),
        source,
    })
}

/// Saves the configuration to the specified path atomically.
pub fn save_to_path(config: &Config, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|source| ConfigError::DirCreateFailed {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let serialized =
        toml::to_string_pretty(config).map_err(|source| ConfigError::SerializeFailed { source })?;

    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, serialized).map_err(|source| ConfigError::WriteFailed {
        path: tmp_path.clone(),
        source,
    })?;

    fs::rename(&tmp_path, path).map_err(|source| ConfigError::WriteFailed {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

/// Saves configuration back to the default resolved path.
pub fn save_config(config: &Config) -> Result<(), ConfigError> {
    let path = resolve_config_path()?;
    save_to_path(config, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_save_config() {
        let dir = tempdir().expect("failed to create tempdir");
        let config_file = dir.path().join("voidctl.toml");

        let mut config = Config::default();
        config
            .jump
            .aliases
            .insert("testalias".to_string(), PathBuf::from("/tmp/testalias"));

        save_to_path(&config, &config_file).expect("failed to save config");
        assert!(config_file.exists());

        let loaded = load_from_path(&config_file).expect("failed to load config");
        assert_eq!(config, loaded);
    }

    #[test]
    fn test_resolve_home_dir() {
        let home = resolve_home_dir();
        assert!(home.is_some());
    }
}
