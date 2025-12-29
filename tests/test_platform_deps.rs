mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

// TODO: Platform-specific dependencies (e.g., [target.'cfg(unix)'.dependencies]) are currently
// not consolidated. They remain in each member's Cargo.toml. We could add support for
// consolidating them into [workspace.target.'cfg(...)'.dependencies] in the future.

#[test]
fn handles_target_specific_dependencies() -> Result<()> {
    let workspace = TestWorkspace::new("test_platform_deps/before")?;

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

    workspace.assert_matches("test_platform_deps/after")?;

    Ok(())
}
