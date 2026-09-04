// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Project command lookup and subprocess execution logic

use crate::config::Config;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

/// Error conditions encountered during command resolution or execution.
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("Unknown jump alias: '{alias}'")]
    AliasNotFound { alias: String },
    #[error("Target directory '{path}' does not exist")]
    DirectoryNotFound { path: PathBuf },
    #[error("No commands configured for alias '{alias}'")]
    NoCommandsFound { alias: String },
    #[error("Command '{cmd_name}' not found for alias '{alias}'. Available: {available}")]
    CommandNotFound {
        alias: String,
        cmd_name: String,
        available: String,
    },
    #[error("Failed to execute command '{command}': {source}")]
    SpawnFailed {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Command terminated by signal")]
    TerminatedBySignal,
}

/// Registers or updates a named command under a project alias.
pub fn add_command(config: &mut Config, alias: &str, cmd_name: &str, shell_cmd: &str) {
    let commands = config.jump.commands.entry(alias.to_string()).or_default();
    commands.insert(cmd_name.to_string(), shell_cmd.to_string());
}

/// Lists all registered commands for a project alias.
#[must_use]
pub fn list_commands<'a>(config: &'a Config, alias: &str) -> Option<Vec<(&'a str, &'a str)>> {
    config.jump.commands.get(alias).map(|cmds| {
        let mut list: Vec<(&'a str, &'a str)> =
            cmds.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        list.sort_by_key(|(k, _)| *k);
        list
    })
}

/// Resolves the project directory and specific command string.
pub fn resolve_command(
    config: &Config,
    alias: &str,
    cmd_name: Option<&str>,
) -> Result<(PathBuf, String), RunnerError> {
    let target_dir = config
        .jump
        .aliases
        .get(alias)
        .ok_or_else(|| RunnerError::AliasNotFound {
            alias: alias.to_string(),
        })?
        .clone();

    if !target_dir.exists() {
        return Err(RunnerError::DirectoryNotFound { path: target_dir });
    }

    let cmds = config
        .jump
        .commands
        .get(alias)
        .ok_or_else(|| RunnerError::NoCommandsFound {
            alias: alias.to_string(),
        })?;

    let command_str = pick_command(cmds, alias, cmd_name)?;
    Ok((target_dir, command_str.to_string()))
}

/// Selects a command by name, or defaults if only one exists or "default"/"run" is present.
fn pick_command<'a>(
    cmds: &'a HashMap<String, String>,
    alias: &str,
    cmd_name: Option<&str>,
) -> Result<&'a String, RunnerError> {
    if let Some(name) = cmd_name {
        cmds.get(name).ok_or_else(|| {
            let available = cmds.keys().cloned().collect::<Vec<_>>().join(", ");
            RunnerError::CommandNotFound {
                alias: alias.to_string(),
                cmd_name: name.to_string(),
                available,
            }
        })
    } else if cmds.len() == 1 {
        Ok(cmds.values().next().expect("cmds has exactly 1 element"))
    } else if let Some(def) = cmds.get("default").or_else(|| cmds.get("run")) {
        Ok(def)
    } else {
        let available = cmds.keys().cloned().collect::<Vec<_>>().join(", ");
        Err(RunnerError::CommandNotFound {
            alias: alias.to_string(),
            cmd_name: "<unspecified>".to_string(),
            available,
        })
    }
}

/// Spawns the command inside target_dir using $SHELL or sh with stdio inherited.
pub fn execute_command(target_dir: &Path, command_str: &str) -> Result<i32, RunnerError> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut child = Command::new(&shell)
        .arg("-c")
        .arg(command_str)
        .current_dir(target_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| RunnerError::SpawnFailed {
            command: command_str.to_string(),
            source,
        })?;

    let status = child.wait().map_err(|source| RunnerError::SpawnFailed {
        command: command_str.to_string(),
        source,
    })?;

    status.code().ok_or(RunnerError::TerminatedBySignal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_runner_resolution() {
        let dir = tempdir().expect("failed to create tempdir");
        let mut config = Config::default();
        config
            .jump
            .aliases
            .insert("prj".to_string(), dir.path().to_path_buf());
        add_command(&mut config, "prj", "test", "cargo test");

        let (resolved_dir, cmd) = resolve_command(&config, "prj", Some("test")).unwrap();
        assert_eq!(resolved_dir, dir.path());
        assert_eq!(cmd, "cargo test");

        let list = list_commands(&config, "prj").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], ("test", "cargo test"));
    }
}
