// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Integration tests for voidctl jump subsystem

use std::path::PathBuf;
use tempfile::tempdir;
use voidctl::config::Config;
use voidctl::jump::{JumpError, add_alias, execute_jump, list_aliases, resolve_alias};

#[test]
fn test_jump_lifecycle() {
    let dir = tempdir().expect("tempdir");
    let mut config = Config::default();

    assert!(resolve_alias(&config, "mosaic").is_none());

    let target_path = dir.path().to_path_buf();
    add_alias(&mut config, "mosaic".to_string(), target_path.clone());

    assert_eq!(
        resolve_alias(&config, "mosaic"),
        Some(target_path.as_path())
    );

    let list = list_aliases(&config);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].0, "mosaic");
    assert_eq!(list[0].1, target_path.as_path());

    let jumped = execute_jump(&config, "mosaic").expect("execute jump");
    assert_eq!(jumped, target_path);

    let nonexistent = execute_jump(&config, "not_found");
    assert!(matches!(nonexistent, Err(JumpError::AliasNotFound { .. })));

    let missing_path = PathBuf::from("/nonexistent/directory/path/12345");
    add_alias(&mut config, "dead".to_string(), missing_path);
    let dead_jump = execute_jump(&config, "dead");
    assert!(matches!(dead_jump, Err(JumpError::PathNotFound { .. })));
}
