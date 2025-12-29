mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
use test_helpers::TestWorkspace;

#[test]
fn fails_when_consolidation_possible() -> Result<()> {
    let workspace = TestWorkspace::new("test_default/before")?;

    let result = workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: true,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    });

    // Check mode should return Err when consolidation is possible
    assert!(result.is_err(), "Expected error but got Ok");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Check failed"),
        "Expected error to contain 'Check failed' but got: {}", err);

    // Verify no files were modified
    let root_content = fs::read_to_string(workspace.path.join("Cargo.toml"))?;
    assert!(!root_content.contains("[workspace.dependencies]"));

    Ok(())
}

#[test]
fn passes_when_no_consolidation_needed() -> Result<()> {
    let workspace = TestWorkspace::new("test_check_passes/before")?;

    let result = workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: true,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    });

    if let Err(e) = &result {
        eprintln!("Error: {}", e);
    }
    assert!(
        result.is_ok(),
        "Check should pass when no consolidation needed"
    );

    Ok(())
}

#[test]
fn fails_when_conflicts_cannot_be_resolved() -> Result<()> {
    let workspace = TestWorkspace::new("test_version_conflict/before")?;

    let result = workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: true,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Fail,
        output_format: OutputFormat::Text,
        output_callback: None,
    });

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Check failed"));

    Ok(())
}
