mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat};
use std::fs;
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

    // Capture state after first run
    let root_after_first = fs::read_to_string(workspace.path.join("Cargo.toml"))?;
    let member1_after_first = fs::read_to_string(workspace.path.join("member1/Cargo.toml"))?;
    let member2_after_first = fs::read_to_string(workspace.path.join("member2/Cargo.toml"))?;

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

    // Capture state after second run
    let root_after_second = fs::read_to_string(workspace.path.join("Cargo.toml"))?;
    let member1_after_second = fs::read_to_string(workspace.path.join("member1/Cargo.toml"))?;
    let member2_after_second = fs::read_to_string(workspace.path.join("member2/Cargo.toml"))?;

    // Should be identical
    assert_eq!(
        root_after_first, root_after_second,
        "Root Cargo.toml changed on second run"
    );
    assert_eq!(
        member1_after_first, member1_after_second,
        "Member1 Cargo.toml changed on second run"
    );
    assert_eq!(
        member2_after_first, member2_after_second,
        "Member2 Cargo.toml changed on second run"
    );

    Ok(())
}
