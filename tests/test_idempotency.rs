mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

#[test]
fn running_twice_is_idempotent() -> Result<()> {
    let workspace = TestWorkspace::new("test_idempotency/before")?;

    // Run once
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

    // Verify first run produced expected output
    workspace.assert_matches("test_idempotency/after")?;

    // Run again
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

    // Verify second run produced identical output
    workspace.assert_matches("test_idempotency/after")?;

    Ok(())
}
