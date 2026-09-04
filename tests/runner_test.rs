// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Integration tests for voidctl runner subsystem

use std::fs;
use tempfile::tempdir;
use voidctl::config::Config;
use voidctl::runner::{RunnerError, add_command, execute_command, list_commands, resolve_command};

#[test]
fn test_runner_execution_flow() {
    let dir = tempdir().expect("tempdir");
    let marker_file = dir.path().join("output.txt");

    let mut config = Config::default();
    config
        .jump
        .aliases
        .insert("prj".to_string(), dir.path().to_path_buf());

    add_command(
        &mut config,
        "prj",
        "create_marker",
        "echo 'voidctl_test' > output.txt",
    );

    let cmds = list_commands(&config, "prj").expect("list commands");
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].0, "create_marker");

    let (exec_dir, cmd) =
        resolve_command(&config, "prj", Some("create_marker")).expect("resolve command");
    assert_eq!(exec_dir, dir.path());

    let exit_code = execute_command(&exec_dir, &cmd).expect("execute command");
    assert_eq!(exit_code, 0);

    assert!(marker_file.exists());
    let content = fs::read_to_string(&marker_file).expect("read marker");
    assert_eq!(content.trim(), "voidctl_test");

    let missing = resolve_command(&config, "prj", Some("unknown"));
    assert!(matches!(missing, Err(RunnerError::CommandNotFound { .. })));
}
