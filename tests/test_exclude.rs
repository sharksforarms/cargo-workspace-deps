mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
use test_helpers::TestWorkspace;

#[test]
fn skips_excluded_dependencies() -> Result<()> {
    let workspace = TestWorkspace::new("test_exclude/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: vec!["serde".to_string()],
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    // Verify serde was NOT moved to workspace
    let root_content = fs::read_to_string(workspace.path.join("Cargo.toml"))?;
    assert!(!root_content.contains("[workspace.dependencies]") || !root_content.contains("serde"));

    // Verify anyhow WAS moved to workspace
    assert!(root_content.contains("anyhow"));

    Ok(())
}
