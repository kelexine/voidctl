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
    #[error("Unsupported shell: '{shell}'. Supported shells: zsh, bash, fish")]
    UnsupportedShell { shell: String },
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

/// Generates shell wrapper script and dynamic alias completion for `j`.
pub fn generate_init_script(shell: &str) -> Result<String, JumpError> {
    match shell.to_lowercase().as_str() {
        "zsh" => Ok(r#"# voidctl jump wrapper for zsh
j() {
    if [ "$#" -eq 0 ]; then
        voidctl jump --list
        return $?
    fi
    local target
    target="$(voidctl jump "$@")" && cd "$target"
}

_j() {
    local -a aliases
    aliases=(${(f)"$(voidctl jump --list 2>/dev/null | awk '{print $1}')"})
    _describe 'jump alias' aliases
}
compdef _j j 2>/dev/null || true
"#
        .to_string()),
        "bash" => Ok(r#"# voidctl jump wrapper for bash
j() {
    if [ "$#" -eq 0 ]; then
        voidctl jump --list
        return $?
    fi
    local target
    target="$(voidctl jump "$@")" && cd "$target"
}

_j() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local aliases
    aliases="$(voidctl jump --list 2>/dev/null | awk '{print $1}')"
    COMPREPLY=( $(compgen -W "${aliases}" -- "${cur}") )
}
complete -F _j j
"#
        .to_string()),
        "fish" => Ok(r#"# voidctl jump wrapper for fish
function j --description 'Jump to registered project alias using voidctl'
    if test (count $argv) -eq 0
        voidctl jump --list
        return $status
    end
    set -l target (voidctl jump $argv[1])
    if test $status -eq 0; and test -n "$target"
        cd $target
    end
end

complete -c j -f -a "(voidctl jump --list 2>/dev/null | awk '{print \$1}')"
"#
        .to_string()),
        _ => Err(JumpError::UnsupportedShell {
            shell: shell.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_init_script() {
        let zsh = generate_init_script("zsh").expect("zsh init script");
        assert!(zsh.contains("j()"));
        assert!(zsh.contains("compdef _j j"));

        let bash = generate_init_script("bash").expect("bash init script");
        assert!(bash.contains("complete -F _j j"));

        let fish = generate_init_script("fish").expect("fish init script");
        assert!(fish.contains("complete -c j"));

        assert!(generate_init_script("tcsh").is_err());
    }
}
