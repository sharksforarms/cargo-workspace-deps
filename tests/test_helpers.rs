#![allow(dead_code)]

use anyhow::Result;
use cargo_workspace_deps::{Config, run};
use std::fs;
use std::path::{Path, PathBuf};

pub struct TestWorkspace {
    pub path: PathBuf,
    _temp_dir: tempfile::TempDir,
}

impl TestWorkspace {
    /// Create a new test workspace from a fixture directory
    pub fn new(fixture_path: &str) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let workspace_path = temp_dir.path().join("workspace");

        // Copy fixture to temp directory
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture_path);

        copy_dir_recursive(&fixture_dir, &workspace_path)?;

        Ok(Self {
            path: workspace_path,
            _temp_dir: temp_dir,
        })
    }

    /// Run the tool with the given config
    pub fn run(&self, config: Config) -> Result<()> {
        run(config)
    }

    /// Assert that workspace matches expected fixture
    pub fn assert_matches(&self, expected_fixture: &str) -> Result<()> {
        let expected_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(expected_fixture);

        let differences = compare_dirs(&self.path, &expected_dir)?;

        if !differences.is_empty() {
            anyhow::bail!(
                "Test failed! Differences found:\n{}",
                differences.join("\n")
            );
        }

        Ok(())
    }
}

/// Copy a directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

/// Compare two files for equality
fn files_equal(path1: &Path, path2: &Path) -> Result<bool> {
    let content1 = fs::read_to_string(path1)?;
    let content2 = fs::read_to_string(path2)?;

    // Normalize line endings and trim trailing whitespace
    let normalize = |s: &str| -> String {
        s.lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(normalize(&content1) == normalize(&content2))
}

/// Compare two directories recursively
fn compare_dirs(dir1: &Path, dir2: &Path) -> Result<Vec<String>> {
    let mut differences = Vec::new();

    // Get all Cargo.toml files in both directories
    let mut files1 = Vec::new();
    let mut files2 = Vec::new();

    collect_cargo_tomls(dir1, dir1, &mut files1)?;
    collect_cargo_tomls(dir2, dir2, &mut files2)?;

    files1.sort();
    files2.sort();

    if files1 != files2 {
        differences.push(format!(
            "Different file structure: {:?} vs {:?}",
            files1, files2
        ));
        return Ok(differences);
    }

    // Compare each file
    for rel_path in &files1 {
        let file1 = dir1.join(rel_path);
        let file2 = dir2.join(rel_path);

        if !files_equal(&file1, &file2)? {
            differences.push(format!("Files differ: {}", rel_path.display()));

            let content1 = fs::read_to_string(&file1)?;
            let content2 = fs::read_to_string(&file2)?;

            differences.push(format!("Expected:\n{}", content2));
            differences.push(format!("Got:\n{}", content1));
        }
    }

    Ok(differences)
}

/// Recursively collect all Cargo.toml files relative to root
fn collect_cargo_tomls(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    use anyhow::Context;

    for entry in fs::read_dir(current)
        .with_context(|| format!("Failed to read directory: {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_cargo_tomls(root, &path, files)?;
        } else if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml"))
            && let Ok(rel_path) = path.strip_prefix(root)
        {
            files.push(rel_path.to_path_buf());
        }
    }
    Ok(())
}
