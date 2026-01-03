mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

#[test]
fn excludes_members_by_glob_pattern() -> Result<()> {
    let workspace = TestWorkspace::new("test_exclude_members/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: vec![glob::Pattern::new("test-*").unwrap()],
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_exclude_members/after")?;

    Ok(())
}
