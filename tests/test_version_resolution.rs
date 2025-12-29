mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat, VersionResolutionStrategy};
use test_helpers::TestWorkspace;

#[test]
fn highest_strategy_uses_highest_version() -> Result<()> {
    let workspace = TestWorkspace::new("test_version_resolution/before")?;

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
        version_resolution_strategy: VersionResolutionStrategy::Highest,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_version_resolution/after_highest")?;

    Ok(())
}

#[test]
fn lowest_strategy_uses_lowest_version() -> Result<()> {
    let workspace = TestWorkspace::new("test_version_resolution/before")?;

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
        version_resolution_strategy: VersionResolutionStrategy::Lowest,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_version_resolution/after_lowest")?;

    Ok(())
}

#[test]
fn highest_compatible_resolves_to_compatible_version() -> Result<()> {
    let workspace = TestWorkspace::new("test_version_resolution/before")?;

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
        version_resolution_strategy: VersionResolutionStrategy::HighestCompatible,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_version_resolution/after_highest_compatible")?;

    Ok(())
}
