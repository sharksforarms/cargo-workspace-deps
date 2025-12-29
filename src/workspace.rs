use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;
use std::path::PathBuf;

#[derive(Debug)]
pub struct WorkspaceInfo {
    pub root_manifest: PathBuf,
    pub members: Vec<MemberInfo>,
}

#[derive(Debug)]
pub struct MemberInfo {
    pub name: String,
    pub manifest_path: PathBuf,
}

/// Discover the workspace structure using the `cargo metadata` command
pub fn discover_workspace(workspace_path: Option<&std::path::Path>) -> Result<WorkspaceInfo> {
    let mut cmd = MetadataCommand::new();
    // Skip dependency resolution to avoid package cache lock
    // (we only need workspace structure)
    cmd.no_deps();

    if let Some(path) = workspace_path {
        cmd.current_dir(path);
    }

    let metadata = cmd.exec().context("Failed to run cargo metadata")?;

    let root_manifest = metadata
        .workspace_root
        .join("Cargo.toml")
        .into_std_path_buf();

    let members: Vec<MemberInfo> = metadata
        .workspace_packages()
        .iter()
        .map(|pkg| MemberInfo {
            name: pkg.name.to_string(),
            manifest_path: pkg.manifest_path.clone().into_std_path_buf(),
        })
        .collect();

    if members.is_empty() {
        anyhow::bail!("No workspace members found. Is this a workspace?");
    }

    Ok(WorkspaceInfo {
        root_manifest,
        members,
    })
}

impl WorkspaceInfo {
    /// Filter out workspace members matching glob patterns
    pub fn filter_by_patterns(&mut self, patterns: &[String]) -> usize {
        if patterns.is_empty() {
            return 0;
        }

        let original_count = self.members.len();

        self.members.retain(|member| {
            !patterns.iter().any(|pattern| {
                glob::Pattern::new(pattern)
                    .map(|p| p.matches(&member.name))
                    .unwrap_or(false)
            })
        });

        original_count - self.members.len()
    }
}
