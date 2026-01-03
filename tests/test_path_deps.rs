mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

// TODO: Path deps are currently not consolidated. They remain in each member's Cargo.toml.
// For now, we just verify that we don't touch them.

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

    workspace.assert_matches("test_mixed_version_path/after")?;

    Ok(())
}
