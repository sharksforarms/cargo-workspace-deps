mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
use test_helpers::TestWorkspace;

#[test]
fn skips_path_dependencies() -> Result<()> {
    let workspace = TestWorkspace::new("test_path_deps/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    // Verify path deps were NOT moved to workspace
    let root_content = fs::read_to_string(workspace.path.join("Cargo.toml"))?;

    // Should consolidate serde (version only)
    assert!(root_content.contains("serde"));

    // Should NOT consolidate my-local-crate (has path)
    assert!(!root_content.contains("my-local-crate"));

    workspace.assert_matches("test_path_deps/after")?;

    Ok(())
}

#[test]
fn skips_mixed_version_and_path() -> Result<()> {
    let workspace = TestWorkspace::new("test_mixed_version_path/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    let root_content = fs::read_to_string(workspace.path.join("Cargo.toml"))?;

    // Should NOT consolidate deps with both version and path
    assert!(!root_content.contains("my-crate"));

    workspace.assert_matches("test_mixed_version_path/after")?;

    Ok(())
}
