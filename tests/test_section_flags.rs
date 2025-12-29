mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
use test_helpers::TestWorkspace;

#[test]
fn skips_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_no_dependencies/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: false,
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

    workspace.assert_matches("test_no_dependencies/after")?;

    Ok(())
}

#[test]
fn skips_dev_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_no_dependencies/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: false,
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

    // Should consolidate serde (dependencies) and cc (build-dependencies)
    assert!(root_content.contains("serde = \"1.0\""), "serde should be consolidated");
    assert!(root_content.contains("cc = \"1.0\""), "cc should be consolidated");

    // Should NOT consolidate rstest (dev-dependencies)
    assert!(!root_content.contains("rstest"), "rstest should not be consolidated");

    Ok(())
}

#[test]
fn skips_build_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_no_dependencies/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: false,
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

    // Should consolidate serde (dependencies) and rstest (dev-dependencies)
    assert!(root_content.contains("serde = \"1.0\""), "serde should be consolidated");
    assert!(root_content.contains("rstest = \"0.23\""), "rstest should be consolidated");

    // Should NOT consolidate cc (build-dependencies)
    assert!(!root_content.contains("cc"), "cc should not be consolidated");

    Ok(())
}

#[test]
fn skips_all_when_all_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_no_dependencies/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: false,
        process_dev_dependencies: false,
        process_build_dependencies: false,
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

    // Should not consolidate anything
    assert!(!root_content.contains("[workspace.dependencies]"), "No workspace.dependencies should be added");
    assert!(!root_content.contains("[workspace.dev-dependencies]"), "No workspace.dev-dependencies should be added");
    assert!(!root_content.contains("[workspace.build-dependencies]"), "No workspace.build-dependencies should be added");

    Ok(())
}
