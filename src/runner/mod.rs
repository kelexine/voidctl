// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Runner subsystem interface for project command execution

pub mod exec;

pub use exec::{RunnerError, add_command, execute_command, list_commands, resolve_command};
