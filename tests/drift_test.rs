// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Integration tests for voidctl drift symlink and mode verification

use std::collections::HashMap;
use std::fs::{self, File};
use std::os::unix::fs::{PermissionsExt, symlink};
use tempfile::tempdir;
use voidctl::drift::links::{LinkStatus, verify_symlinks};

#[test]
fn test_drift_symlink_comprehensive() {
    let dir = tempdir().expect("tempdir");
    let dotfiles = dir.path().join("dotfiles");
    let home = dir.path().join("home");
    fs::create_dir_all(&dotfiles).expect("create dotfiles");
    fs::create_dir_all(&home).expect("create home");

    // 1. Valid link
    let valid_src = dotfiles.join("validrc");
    File::create(&valid_src).expect("create validrc");
    let valid_target = home.join(".validrc");
    symlink(&valid_src, &valid_target).expect("create symlink");

    // 2. Broken link (source exists in dotfiles, but symlink points to wrong/stale path)
    let broken_src = dotfiles.join("brokenrc");
    File::create(&broken_src).expect("create brokenrc");
    let broken_target = home.join(".brokenrc");
    symlink(dotfiles.join("nowhere"), &broken_target).expect("create broken symlink");

    // 3. Replaced by real file
    let replaced_src = dotfiles.join("replacedrc");
    File::create(&replaced_src).expect("create replacedrc");
    let replaced_target = home.join(".replacedrc");
    File::create(&replaced_target).expect("create real file");

    // 4. Missing target link
    let missing_src = dotfiles.join("missingrc");
    File::create(&missing_src).expect("create missingrc");

    // 5. Mode drift (e.g. ssh config with group/other write permissions)
    let ssh_dir = dotfiles.join("ssh");
    fs::create_dir_all(&ssh_dir).expect("create ssh dir");
    let ssh_src = ssh_dir.join("config");
    let ssh_file = File::create(&ssh_src).expect("create ssh config");
    let mut perms = ssh_file.metadata().expect("meta").permissions();
    perms.set_mode(0o666);
    fs::set_permissions(&ssh_src, perms).expect("set 666 mode");

    let home_ssh = home.join(".ssh");
    fs::create_dir_all(&home_ssh).expect("create home ssh dir");
    let ssh_target = home_ssh.join("config");
    symlink(&ssh_src, &ssh_target).expect("create ssh symlink");

    let mut map = HashMap::new();
    map.insert("validrc".to_string(), ".validrc".to_string());
    map.insert("brokenrc".to_string(), ".brokenrc".to_string());
    map.insert("replacedrc".to_string(), ".replacedrc".to_string());
    map.insert("missingrc".to_string(), ".missingrc".to_string());
    map.insert("ssh/config".to_string(), ".ssh/config".to_string());

    let results = verify_symlinks(&dotfiles, &home, &map);

    let find_status = |target: &str| {
        results
            .iter()
            .find(|r| r.target_rel == target)
            .map(|r| &r.status)
            .expect("must find target")
    };

    assert_eq!(find_status(".validrc"), &LinkStatus::Valid);
    assert!(matches!(
        find_status(".brokenrc"),
        LinkStatus::Broken { .. }
    ));
    assert_eq!(find_status(".replacedrc"), &LinkStatus::ReplacedByRealFile);
    assert_eq!(find_status(".missingrc"), &LinkStatus::Missing);
    assert!(matches!(
        find_status(".ssh/config"),
        LinkStatus::PermissionDrift { .. }
    ));
}
