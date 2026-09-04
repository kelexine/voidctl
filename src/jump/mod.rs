// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Subcommand handlers and pure resolution logic for voidctl jump

pub mod registry;

pub use registry::{add_alias, list_aliases, resolve_alias};

use crate::config::Config;
use std::path::PathBuf;
use thiserror::Error;

/// Error conditions when resolving or mutating jump aliases.
#[derive(Debug, Error)]
pub enum JumpError {
    #[error("Unknown jump alias: '{alias}'")]
    AliasNotFound { alias: String },
    #[error("Target path '{path}' does not exist")]
    PathNotFound { path: PathBuf },
}

/// Resolves an alias, validating that the target directory exists on disk.
pub fn execute_jump(config: &Config, alias: &str) -> Result<PathBuf, JumpError> {
    let path = resolve_alias(config, alias)
        .ok_or_else(|| JumpError::AliasNotFound {
            alias: alias.to_string(),
        })?
        .to_path_buf();

    if !path.exists() {
        return Err(JumpError::PathNotFound { path });
    }

    Ok(path)
}
