mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
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
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Check failed"));

    // Verify no files were modified
    workspace.assert_matches("test_default/before")?;

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

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Check failed"));

    // Verify no files were modified
    workspace.assert_matches("test_version_conflict/before")?;

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

    result.expect("should not fail");

    // Verify no files were modified
    workspace.assert_matches("test_check_passes/before")?;

    Ok(())
}
