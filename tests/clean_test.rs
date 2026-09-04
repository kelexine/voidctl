// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Integration tests for voidctl clean subsystem and classifiers

use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;
use voidctl::clean::classifier::CleanCategory;
use voidctl::clean::scan_hygiene;
use voidctl::config::CleanConfig;

#[test]
fn test_clean_scan_hierarchies() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // 1. Rust project structure with target/
    let rust_project = root.join("rust_prj");
    fs::create_dir_all(&rust_project).expect("create rust project");
    File::create(rust_project.join("Cargo.toml")).expect("create Cargo.toml");
    let target_dir = rust_project.join("target");
    fs::create_dir_all(&target_dir).expect("create target");
    let mut artifact_file = File::create(target_dir.join("libapp.rlib")).expect("create artifact");
    artifact_file
        .write_all(&[0u8; 1024])
        .expect("write dummy bytes");

    // 2. Python bytecode file
    let pyc_file = root.join("app.cpython-314.pyc");
    let mut pyc = File::create(&pyc_file).expect("create pyc");
    pyc.write_all(&[0u8; 512]).expect("write pyc bytes");

    let clean_config = CleanConfig {
        scan_roots: vec![root.to_path_buf()],
        age_threshold_days: 7,
        exclude: vec![".git".to_string()],
        extra_scan_roots: Vec::new(),
    };

    let report = scan_hygiene(&clean_config);

    assert!(!report.items.is_empty());
    let target_item = report
        .items
        .iter()
        .find(|i| i.path == target_dir)
        .expect("target_dir should be classified");
    assert_eq!(target_item.category, CleanCategory::Artifacts);
    assert!(target_item.size_bytes >= 1024);

    let pyc_item = report
        .items
        .iter()
        .find(|i| i.path == pyc_file)
        .expect("pyc should be classified");
    assert_eq!(pyc_item.category, CleanCategory::Artifacts);
}
