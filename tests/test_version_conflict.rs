mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
use test_helpers::TestWorkspace;

#[test]
fn detects_incompatible_version_conflict() -> Result<()> {
    let workspace = TestWorkspace::new("test_version_conflict/before")?;

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
        version_resolution_strategy:
            cargo_workspace_deps::VersionResolutionStrategy::HighestCompatible,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    // Should not be consolidated - 0.2 and 1.0 are incompatible
    let root_content = fs::read_to_string(workspace.path.clone().join("Cargo.toml"))?;
    assert!(
        !root_content.contains("tokio"),
        "tokio should not be consolidated"
    );

    workspace.assert_matches("test_version_conflict/after")?;

    Ok(())
}
