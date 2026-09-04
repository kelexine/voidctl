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

    assert!(!report.targets.is_empty());
    let target_entry = report
        .targets
        .iter()
        .find(|t| t.path == target_dir)
        .expect("target_dir should be classified in targets");
    assert_eq!(target_entry.category, CleanCategory::Artifacts);
    assert!(target_entry.size_bytes >= 1024);

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

#[test]
fn test_clean_scan_node_modules() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let js_project = root.join("node_prj");
    fs::create_dir_all(&js_project).expect("create js project");
    File::create(js_project.join("package.json")).expect("create package.json");
    let nm_dir = js_project.join("node_modules");
    fs::create_dir_all(&nm_dir).expect("create node_modules");
    File::create(nm_dir.join("pkg.js")).expect("create pkg.js");

    let clean_config = CleanConfig {
        scan_roots: vec![root.to_path_buf()],
        age_threshold_days: 7,
        exclude: vec![".git".to_string()],
        extra_scan_roots: Vec::new(),
    };

    let report = scan_hygiene(&clean_config);
    let target = report.targets.iter().find(|t| t.path == nm_dir);
    assert!(target.is_some());
    assert_eq!(
        target.expect("target exists").category,
        CleanCategory::Artifacts
    );
}
