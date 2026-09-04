// Author: kelexine <https://github.com/kelexine>
// Date: 2026-09-04
// Purpose: Build artifact classifier (Cargo, Node, Python) for system hygiene

use std::path::Path;

/// Checks if a directory path corresponds to a known build artifact tree.
#[must_use]
pub fn is_build_artifact_dir(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|s| s.to_str()) {
        Some(name) => name,
        None => return false,
    };

    match file_name {
        "target" => has_marker_sibling_or_tag(path, "Cargo.toml", "CACHEDIR.TAG"),
        "node_modules" => has_marker_sibling(path, "package.json"),
        "__pycache__" => true,
        ".pytest_cache" => true,
        ".mypy_cache" => true,
        ".ruff_cache" => true,
        _ => false,
    }
}

/// Checks if a file path is a standalone compiled artifact (e.g. *.pyc).
#[must_use]
pub fn is_build_artifact_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pyc"))
}

/// Checks if the parent contains a marker file, or if the directory contains a tag.
fn has_marker_sibling_or_tag(dir: &Path, sibling_marker: &str, child_tag: &str) -> bool {
    if dir.join(child_tag).exists() {
        return true;
    }
    has_marker_sibling(dir, sibling_marker)
}

/// Checks if the parent directory contains a marker file (e.g. Cargo.toml).
fn has_marker_sibling(dir: &Path, marker: &str) -> bool {
    dir.parent()
        .map(|parent| parent.join(marker).exists())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_cargo_target_detection() {
        let dir = tempdir().expect("tempdir");
        let cargo_toml = dir.path().join("Cargo.toml");
        File::create(&cargo_toml).expect("create Cargo.toml");

        let target_dir = dir.path().join("target");
        std::fs::create_dir(&target_dir).expect("create target");

        assert!(is_build_artifact_dir(&target_dir));
    }

    #[test]
    fn test_node_modules_detection() {
        let dir = tempdir().expect("tempdir");
        let pkg_json = dir.path().join("package.json");
        File::create(&pkg_json).expect("create package.json");

        let nm_dir = dir.path().join("node_modules");
        std::fs::create_dir(&nm_dir).expect("create node_modules");

        assert!(is_build_artifact_dir(&nm_dir));
    }
}
