mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

#[test]
fn preserves_comments_and_formatting() -> Result<()> {
    let workspace = TestWorkspace::new("test_format_preservation/before")?;

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

    workspace.assert_matches("test_format_preservation/after")?;

    Ok(())
}
