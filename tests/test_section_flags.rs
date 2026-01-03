mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use test_helpers::TestWorkspace;

#[test]
fn skips_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_section_flags/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: false,
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

    workspace.assert_matches("test_section_flags/after_no_dependencies")?;

    Ok(())
}

#[test]
fn skips_dev_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_section_flags/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: false,
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

    workspace.assert_matches("test_section_flags/after_no_dev_dependencies")?;

    Ok(())
}

#[test]
fn skips_build_dependencies_when_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_section_flags/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: false,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_section_flags/after_no_build_dependencies")?;

    Ok(())
}

#[test]
fn skips_all_when_all_disabled() -> Result<()> {
    let workspace = TestWorkspace::new("test_section_flags/before")?;

    workspace.run(Config {
        fix: true,
        process_dependencies: false,
        process_dev_dependencies: false,
        process_build_dependencies: false,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: cargo_workspace_deps::VersionResolutionStrategy::Skip,
        output_format: OutputFormat::Text,
        output_callback: None,
    })?;

    workspace.assert_matches("test_section_flags/after_all_disabled")?;

    Ok(())
}
