mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

/// Test that version conflicts are detected and resolved across different sections
/// (e.g., tokio "1.0" in [dependencies] vs tokio "2.0" in [dev-dependencies])
#[test]
fn resolves_version_conflict_across_sections() -> Result<()> {
    let workspace = TestWorkspace::new("test_cross_section_conflict/before")?;

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
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Highest,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_cross_section_conflict/after")?;

    Ok(())
}
