use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepSection {
    Dependencies,
    DevDependencies,
    BuildDependencies,
}

impl DepSection {
    pub fn as_str(&self) -> &str {
        match self {
            DepSection::Dependencies => "dependencies",
            DepSection::DevDependencies => "dev-dependencies",
            DepSection::BuildDependencies => "build-dependencies",
        }
    }

    pub fn workspace_key(&self) -> &str {
        match self {
            DepSection::Dependencies => "workspace.dependencies",
            DepSection::DevDependencies => "workspace.dev-dependencies",
            DepSection::BuildDependencies => "workspace.build-dependencies",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySpec {
    pub name: String,
    pub version: String,
    pub section: DepSection,
    pub package: Option<String>,
    pub registry: Option<String>,
    pub default_features: Option<bool>,
}

/// Parsed workspace dependency information
#[derive(Debug, Clone)]
pub struct WorkspaceDep {
    pub name: String,
    pub version: String,
    pub section: DepSection,
    pub package: Option<String>,
    pub registry: Option<String>,
    pub default_features: Option<bool>,
}

/// All parsed dependency data from workspace and members
pub struct WorkspaceData {
    pub workspace_deps: HashMap<(String, DepSection), WorkspaceDep>,
    pub member_deps: HashMap<String, Vec<DependencySpec>>,
    pub workspace_refs: Vec<(String, DepSection)>, // Deps already using { workspace = true }
}

/// Key for grouping dependencies that should share a workspace entry
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct WorkspaceDepKey {
    name: String,
    section: DepSection,
    package: Option<String>,
    registry: Option<String>,
    // Note: default-features is NOT in the key because we want to detect conflicts
}

#[derive(Debug)]
pub struct DependencyAnalysis {
    /// Dependencies that will be (or are already) consolidated to workspace.dependencies
    /// Includes both newly consolidated deps and resolved version conflicts
    pub common_deps: Vec<CommonDependency>,

    /// Dependencies with version conflicts that could not be resolved
    pub conflicts: Vec<ConflictingDependency>,

    /// Workspace dependencies that are not used by any member
    pub unused_workspace_deps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommonDependency {
    pub name: String,
    pub version: String,
    pub section: DepSection,
    /// Members that have this dependency (need conversion to { workspace = true })
    pub members: Vec<String>,
    /// Renamed package (e.g., serde_crate = { package = "serde", ... })
    pub package: Option<String>,
    /// Custom registry for private crates
    pub registry: Option<String>,
    /// Whether to disable default features
    pub default_features: Option<bool>,
    /// Original version map if this was resolved from a conflict
    /// None = single version, Some = resolved from multiple versions
    pub resolved_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone)]
pub struct VersionSpec {
    pub version: String,
    pub default_features: Option<bool>,
    pub members: Vec<String>,
}

/// Internal structure for tracking version usage during analysis
#[derive(Debug, Clone, Default)]
struct VersionUsage {
    /// Actual member names that use this version
    members: Vec<String>,
    /// Whether this version is defined in [workspace.dependencies]
    in_workspace: bool,
}

impl VersionUsage {
    /// Convert to Vec<String> for compatibility with version resolver
    fn to_member_list(&self) -> Vec<String> {
        let mut result = self.members.clone();
        if self.in_workspace {
            result.push("workspace".to_string());
        }
        result
    }
}

/// Convert a version map with VersionUsage to one with Vec<String>
fn version_map_to_member_lists(
    version_map: &HashMap<String, VersionUsage>,
) -> HashMap<String, Vec<String>> {
    version_map
        .iter()
        .map(|(version, usage)| (version.clone(), usage.to_member_list()))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    VersionResolution,
    DefaultFeatures,
}

#[derive(Debug, Clone)]
pub struct ConflictingDependency {
    pub name: String,
    pub section: DepSection,
    pub version_specs: Vec<VersionSpec>,
    pub conflict_types: Vec<ConflictType>,
}

/// Helper macro to extract fields from table-like structures (InlineTable or Table)
macro_rules! extract_from_table {
    ($table:expr) => {{
        if $table.contains_key("path") || $table.contains_key("git") {
            return None;
        }
        let version = $table
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;
        let package = $table
            .get("package")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let registry = $table
            .get("registry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default_features = $table.get("default-features").and_then(|v| v.as_bool());
        Some((version, package, registry, default_features))
    }};
}

/// Helper macro to extract optional version fields (for workspace deps)
macro_rules! extract_version_fields {
    ($table:expr) => {{
        let version = $table
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let package = $table
            .get("package")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let registry = $table
            .get("registry")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default_features = $table.get("default-features").and_then(|v| v.as_bool());
        (version, package, registry, default_features)
    }};
}

/// Extract dependency info from TOML item
/// Returns (version, package, registry, default_features) or None if should skip (path or git)
#[allow(clippy::type_complexity)]
fn extract_dep_info(item: &Item) -> Option<(String, Option<String>, Option<String>, Option<bool>)> {
    match item {
        Item::Value(val) if val.is_inline_table() => val
            .as_inline_table()
            .and_then(|table| extract_from_table!(table)),
        Item::Value(val) => val.as_str().map(|s| (s.to_string(), None, None, None)),
        Item::Table(table) => extract_from_table!(table),
        _ => None,
    }
}

/// Parse dependencies from a Cargo.toml file
/// Returns (explicit_deps, workspace_refs)
/// - explicit_deps: deps with explicit versions (need consolidation)
/// - workspace_refs: deps already using { workspace = true }
#[allow(clippy::type_complexity)]
pub fn parse_dependencies(
    manifest_path: &Path,
    sections: &[DepSection],
) -> Result<(Vec<DependencySpec>, Vec<(String, DepSection)>)> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", manifest_path.display()))?;

    let mut deps = Vec::new();
    let mut workspace_refs = Vec::new();

    for section in sections {
        if let Some(Item::Table(table)) = doc.get(section.as_str()) {
            for (name, item) in table.iter() {
                let uses_workspace = match item {
                    Item::Table(t) => t.contains_key("workspace"),
                    Item::Value(val) if val.is_inline_table() => val
                        .as_inline_table()
                        .map(|t| t.contains_key("workspace"))
                        .unwrap_or(false),
                    _ => false,
                };

                if uses_workspace {
                    workspace_refs.push((name.to_string(), *section));
                    continue;
                }

                if let Some((version, package, registry, default_features)) = extract_dep_info(item)
                {
                    deps.push(DependencySpec {
                        name: name.to_string(),
                        version,
                        section: *section,
                        package,
                        registry,
                        default_features: Some(default_features.unwrap_or(true)),
                    });
                }
            }
        }
    }

    Ok((deps, workspace_refs))
}

/// Parse workspace dependencies from root Cargo.toml
pub fn parse_workspace_dependencies(
    workspace_manifest: &Path,
    sections: &[DepSection],
) -> Result<HashMap<(String, DepSection), WorkspaceDep>> {
    let content = fs::read_to_string(workspace_manifest)
        .with_context(|| format!("Failed to read {}", workspace_manifest.display()))?;

    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", workspace_manifest.display()))?;

    let mut workspace_deps = HashMap::new();

    if let Some(Item::Table(workspace)) = doc.get("workspace") {
        for section in sections {
            let section_key = section.as_str();
            if let Some(Item::Table(deps_table)) = workspace.get(section_key) {
                for (name, item) in deps_table.iter() {
                    let (version, package, registry, default_features) = match item {
                        Item::Value(val) if val.is_inline_table() => val
                            .as_inline_table()
                            .map_or((None, None, None, None), |table| extract_version_fields!(table)),
                        Item::Value(val) => (val.as_str().map(|s| s.to_string()), None, None, None),
                        Item::Table(table) => extract_version_fields!(table),
                        _ => (None, None, None, None),
                    };

                    if let Some(version) = version {
                        let key = (name.to_string(), *section);
                        workspace_deps.insert(
                            key,
                            WorkspaceDep {
                                name: name.to_string(),
                                version,
                                section: *section,
                                package,
                                registry,
                                default_features: Some(default_features.unwrap_or(true)),
                            },
                        );
                    }
                }
            }
        }
    }

    Ok(workspace_deps)
}

/// Parse all workspace data (workspace deps + member deps)
pub fn parse_workspace_data(
    workspace_info: &crate::workspace::WorkspaceInfo,
    sections: &[DepSection],
) -> Result<WorkspaceData> {
    let workspace_deps = parse_workspace_dependencies(&workspace_info.root_manifest, sections)?;

    let mut member_deps = HashMap::new();
    let mut all_workspace_refs = Vec::new();

    for member in &workspace_info.members {
        let (deps, workspace_refs) = parse_dependencies(&member.manifest_path, sections)?;
        if !deps.is_empty() {
            member_deps.insert(member.name.clone(), deps);
        }
        all_workspace_refs.extend(workspace_refs);
    }

    Ok(WorkspaceData {
        workspace_deps,
        member_deps,
        workspace_refs: all_workspace_refs,
    })
}

/// Check if we should consolidate based on workspace presence and member count
fn should_consolidate(has_workspace: bool, member_count: usize, min_members: usize) -> bool {
    (has_workspace && member_count > 0) || (!has_workspace && member_count >= min_members)
}

/// Check if there's a default-features conflict for the given version
fn has_default_features_conflict(
    key: &WorkspaceDepKey,
    version: &str,
    default_features_map: &HashMap<(WorkspaceDepKey, String), Vec<Option<bool>>>,
) -> bool {
    let df_key = (key.clone(), version.to_string());
    let df_values = default_features_map.get(&df_key).cloned().unwrap_or_default();
    let unique_df: std::collections::HashSet<_> = df_values.into_iter().collect();
    unique_df.len() > 1
}

/// Create a ConflictingDependency with the given conflict types
#[allow(clippy::type_complexity)]
fn create_conflict(
    key: &WorkspaceDepKey,
    version_spec_map: &HashMap<WorkspaceDepKey, HashMap<(String, Option<bool>), VersionUsage>>,
    conflict_types: Vec<ConflictType>,
) -> ConflictingDependency {
    let version_specs_map = version_spec_map.get(key).cloned().unwrap_or_default();
    let version_specs = version_specs_map
        .into_iter()
        .map(|((version, default_features), usage)| {
            // Build members list: include actual members, and optionally "workspace" marker
            let mut members = usage.members.clone();
            if usage.in_workspace && members.is_empty() {
                // Only workspace, no member uses this version
                members.push("workspace".to_string());
            } else if usage.in_workspace {
                // Both workspace and members
                members.push("workspace".to_string());
            }
            VersionSpec {
                version,
                default_features,
                members,
            }
        })
        .collect();
    ConflictingDependency {
        name: key.name.clone(),
        section: key.section,
        version_specs,
        conflict_types,
    }
}

/// Analyze all aspects of workspace dependencies in one pass
#[allow(clippy::type_complexity)]
pub fn analyze_workspace(
    data: &WorkspaceData,
    exclude: &[String],
    min_members: usize,
    resolution_strategy: &crate::VersionResolutionStrategy,
) -> Result<DependencyAnalysis> {
    let mut dep_map: HashMap<WorkspaceDepKey, HashMap<String, VersionUsage>> = HashMap::new();
    let mut default_features_map: HashMap<(WorkspaceDepKey, String), Vec<Option<bool>>> =
        HashMap::new();
    let mut version_spec_map: HashMap<WorkspaceDepKey, HashMap<(String, Option<bool>), VersionUsage>> =
        HashMap::new();

    for ((name, section), ws_dep) in &data.workspace_deps {
        let key = WorkspaceDepKey {
            name: name.clone(),
            section: *section,
            package: ws_dep.package.clone(),
            registry: ws_dep.registry.clone(),
        };

        dep_map
            .entry(key.clone())
            .or_default()
            .entry(ws_dep.version.clone())
            .or_default()
            .in_workspace = true;

        default_features_map
            .entry((key.clone(), ws_dep.version.clone()))
            .or_default()
            .push(ws_dep.default_features);

        // Track version spec for conflict reporting
        version_spec_map
            .entry(key)
            .or_default()
            .entry((ws_dep.version.clone(), ws_dep.default_features))
            .or_default()
            .in_workspace = true;
    }

    for (member_name, deps) in &data.member_deps {
        for dep in deps {
            let key = WorkspaceDepKey {
                name: dep.name.clone(),
                section: dep.section,
                package: dep.package.clone(),
                registry: dep.registry.clone(),
            };

            dep_map
                .entry(key.clone())
                .or_default()
                .entry(dep.version.clone())
                .or_default()
                .members
                .push(member_name.clone());

            default_features_map
                .entry((key.clone(), dep.version.clone()))
                .or_default()
                .push(dep.default_features);

            // Track version spec for conflict reporting
            version_spec_map
                .entry(key)
                .or_default()
                .entry((dep.version.clone(), dep.default_features))
                .or_default()
                .members
                .push(member_name.clone());
        }
    }

    let mut common_deps = Vec::new();
    let mut conflicts = Vec::new();

    for (key, version_map) in dep_map {
        if exclude.contains(&key.name) {
            continue;
        }

        let has_workspace = version_map
            .values()
            .any(|usage| usage.in_workspace);

        let all_real_members: Vec<String> = version_map
            .values()
            .flat_map(|usage| usage.members.iter())
            .cloned()
            .collect();

        if version_map.len() == 1 {
            let version = version_map.keys().next().unwrap().clone();

            // Check for default-features conflict
            if has_default_features_conflict(&key, &version, &default_features_map) {
                let conflict = create_conflict(&key, &version_spec_map, vec![ConflictType::DefaultFeatures]);
                conflicts.push(conflict);
                continue;
            }

            let df_key = (key.clone(), version.clone());
            let df_values = default_features_map.get(&df_key).cloned().unwrap_or_default();
            let unique_df: std::collections::HashSet<_> = df_values.into_iter().collect();
            let common_default_features = unique_df.into_iter().next().flatten();

            if should_consolidate(has_workspace, all_real_members.len(), min_members) {
                common_deps.push(CommonDependency {
                    name: key.name,
                    version,
                    section: key.section,
                    members: all_real_members,
                    package: key.package,
                    registry: key.registry,
                    default_features: common_default_features,
                    resolved_from: None,
                });
            }
        } else {
            // Convert VersionUsage map to Vec<String> map for version resolver
            let member_lists_map = version_map_to_member_lists(&version_map);

            match crate::version_resolver::resolve_version_conflict(
                &member_lists_map,
                resolution_strategy,
            ) {
                Ok((resolved_version, _)) => {
                    // Check for default-features conflict after resolving version
                    if has_default_features_conflict(&key, &resolved_version, &default_features_map) {
                        let conflict = create_conflict(&key, &version_spec_map, vec![ConflictType::DefaultFeatures]);
                        conflicts.push(conflict);
                        continue;
                    }

                    let df_key = (key.clone(), resolved_version.clone());
                    let df_values = default_features_map.get(&df_key).cloned().unwrap_or_default();
                    let unique_df: std::collections::HashSet<_> = df_values.into_iter().collect();
                    let common_default_features = unique_df.into_iter().next().flatten();

                    if should_consolidate(has_workspace, all_real_members.len(), min_members) {
                        common_deps.push(CommonDependency {
                            name: key.name.clone(),
                            version: resolved_version,
                            section: key.section,
                            members: all_real_members,
                            package: key.package.clone(),
                            registry: key.registry.clone(),
                            default_features: common_default_features,
                            resolved_from: Some(member_lists_map),
                        });
                    }
                }
                Err(_) => {
                    // Version resolution failed - check if there's also a default-features conflict
                    let mut conflict_types = vec![ConflictType::VersionResolution];

                    // Check if there's also a default-features conflict across ALL versions
                    let all_df_values: Vec<Option<bool>> = default_features_map
                        .iter()
                        .filter(|((k, _), _)| k == &key)
                        .flat_map(|(_, values)| values.clone())
                        .collect();
                    let unique_df: std::collections::HashSet<_> = all_df_values.into_iter().collect();
                    if unique_df.len() > 1 {
                        conflict_types.push(ConflictType::DefaultFeatures);
                    }

                    let conflict = create_conflict(&key, &version_spec_map, conflict_types);
                    conflicts.push(conflict);
                }
            }
        }
    }

    let mut used_deps = std::collections::HashSet::new();

    for common_dep in &common_deps {
        used_deps.insert(format!("{}::{:?}", common_dep.name, common_dep.section));
    }

    for (name, section) in &data.workspace_refs {
        used_deps.insert(format!("{}::{:?}", name, section));
    }

    let unused_workspace_deps: Vec<String> = data
        .workspace_deps
        .iter()
        .filter(|((name, section), _)| !used_deps.contains(&format!("{}::{:?}", name, section)))
        .map(|((name, _), _)| name.clone())
        .collect();

    Ok(DependencyAnalysis {
        common_deps,
        conflicts,
        unused_workspace_deps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temporary Cargo.toml with given content
    fn create_test_manifest(content: &str) -> Result<(TempDir, std::path::PathBuf)> {
        let temp_dir = tempfile::tempdir()?;
        let manifest_path = temp_dir.path().join("Cargo.toml");
        let full_content = format!(
            r#"[package]
name = "test"
version = "0.1.0"
{}"#,
            content
        );
        fs::write(&manifest_path, full_content)?;
        Ok((temp_dir, manifest_path))
    }

    #[rstest]
    #[case::simple_string_version(
        r#"
[dependencies]
serde = "1.0"
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::inline_table_version(
        r#"
[dependencies]
serde = { version = "1.0" }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::table_format_version(
        r#"
[dependencies.serde]
version = "1.0"
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::multiple_dependencies(
        r#"
[dependencies]
serde = "1.0"
anyhow = "1.0"
tokio = { version = "1.0" }
"#,
        vec![DepSection::Dependencies],
        vec![
            DependencySpec {
                name: "serde".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
            DependencySpec {
                name: "anyhow".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
            DependencySpec {
                name: "tokio".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
        ]
    )]
    #[case::renamed_package(
        r#"
[dependencies]
serde_crate = { package = "serde", version = "1.0" }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde_crate".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: Some("serde".into()),
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::custom_registry(
        r#"
[dependencies]
my_crate = { version = "1.0", registry = "my-registry" }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "my_crate".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: Some("my-registry".into()),
            default_features: Some(true),
        }]
    )]
    #[case::dev_dependencies(
        r#"
[dev-dependencies]
rstest = "0.23"
"#,
        vec![DepSection::DevDependencies],
        vec![DependencySpec {
            name: "rstest".into(),
            version: "0.23".into(),
            section: DepSection::DevDependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::build_dependencies(
        r#"
[build-dependencies]
cc = "1.0"
"#,
        vec![DepSection::BuildDependencies],
        vec![DependencySpec {
            name: "cc".into(),
            version: "1.0".into(),
            section: DepSection::BuildDependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::multiple_sections(
        r#"
[dependencies]
serde = "1.0"

[dev-dependencies]
rstest = "0.23"

[build-dependencies]
cc = "1.0"
"#,
        vec![DepSection::Dependencies, DepSection::DevDependencies, DepSection::BuildDependencies],
        vec![
            DependencySpec {
                name: "serde".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
            DependencySpec {
                name: "rstest".into(),
                version: "0.23".into(),
                section: DepSection::DevDependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
            DependencySpec {
                name: "cc".into(),
                version: "1.0".into(),
                section: DepSection::BuildDependencies,
                package: None,
                registry: None,
                default_features: Some(true),
            },
        ]
    )]
    #[case::workspace_deps_skipped(
        r#"
[dependencies]
serde = { workspace = true }
anyhow = "1.0"
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "anyhow".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::path_deps_skipped(
        r#"
[dependencies]
my_local = { path = "../my-local" }
serde = "1.0"
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::git_deps_skipped(
        r#"
[dependencies]
my_git = { git = "https://github.com/example/repo" }
serde = "1.0"
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::empty_section(
        r#"
[dependencies]
"#,
        vec![DepSection::Dependencies],
        vec![]
    )]
    #[case::missing_section(
        "",
        vec![DepSection::Dependencies],
        vec![]
    )]
    #[case::version_with_features(
        r#"
[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::version_with_optional(
        r#"
[dependencies]
serde = { version = "1.0", optional = true }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
                default_features: Some(true),
        }]
    )]
    #[case::version_with_default_features(
        r#"
[dependencies]
serde = { version = "1.0", default-features = false }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "serde".into(),
            version: "1.0".into(),
            section: DepSection::Dependencies,
            package: None,
            registry: None,
            default_features: Some(false),
        }]
    )]
    #[case::complex_dependency(
        r#"
[dependencies]
my_crate = { package = "real-crate", version = "2.0", registry = "custom", features = ["async"], optional = true }
"#,
        vec![DepSection::Dependencies],
        vec![DependencySpec {
            name: "my_crate".into(),
            version: "2.0".into(),
            section: DepSection::Dependencies,
            package: Some("real-crate".into()),
            registry: Some("custom".into()),
                default_features: Some(true),
        }]
    )]
    #[case::path_and_version_skipped(
        r#"
[dependencies]
my_crate = { path = "../local", version = "1.0" }
"#,
        vec![DepSection::Dependencies],
        vec![]
    )]
    #[case::git_with_version_skipped(
        r#"
[dependencies]
my_crate = { git = "https://github.com/example/repo", version = "1.0" }
"#,
        vec![DepSection::Dependencies],
        vec![]
    )]
    fn test_parse_dependencies(
        #[case] toml_content: &str,
        #[case] sections: Vec<DepSection>,
        #[case] expected: Vec<DependencySpec>,
    ) -> Result<()> {
        let (_temp_dir, manifest_path) = create_test_manifest(toml_content)?;

        let (deps, _workspace_refs) = parse_dependencies(&manifest_path, &sections)?;

        // Sort both vectors by name for consistent comparison
        let mut deps = deps;
        let mut expected_specs = expected;
        deps.sort_by(|a, b| a.name.cmp(&b.name));
        expected_specs.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(deps.len(), expected_specs.len());

        for (actual, expected) in deps.iter().zip(expected_specs.iter()) {
            assert_eq!(actual, expected);
        }

        Ok(())
    }

    #[test]
    fn test_invalid_toml() {
        let (_temp_dir, manifest_path) = create_test_manifest("not valid toml [[[").unwrap();
        let result = parse_dependencies(&manifest_path, &[DepSection::Dependencies]);
        assert!(result.is_err(), "Should fail on invalid TOML");
    }

    #[test]
    fn test_missing_file() {
        let result = parse_dependencies(
            std::path::Path::new("/nonexistent/path/Cargo.toml"),
            &[DepSection::Dependencies],
        );
        assert!(result.is_err(), "Should fail on missing file");
    }
}
