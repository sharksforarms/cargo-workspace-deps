use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item};

const WORKSPACE_MARKER: &str = "workspace";

/// Result of parsing dependencies from a Cargo.toml file
#[derive(Debug)]
pub(crate) struct ParsedDependencies {
    /// Dependencies with explicit versions that need consolidation
    pub(crate) explicit_deps: Vec<DependencySpec>,
    /// Dependencies already using { workspace = true }
    pub(crate) workspace_refs: Vec<(String, DepSection)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DepSection {
    Dependencies,
    DevDependencies,
    BuildDependencies,
}

impl DepSection {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            DepSection::Dependencies => "dependencies",
            DepSection::DevDependencies => "dev-dependencies",
            DepSection::BuildDependencies => "build-dependencies",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencySpec {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) section: DepSection,
    pub(crate) package: Option<String>,
    pub(crate) registry: Option<String>,
    pub(crate) default_features: bool,
}

/// All parsed dependency data from workspace and members
pub(crate) struct WorkspaceData {
    pub(crate) workspace_deps: HashMap<String, DependencySpec>,
    pub(crate) member_deps: HashMap<String, Vec<DependencySpec>>,
    // Deps already using { workspace = true }
    pub(crate) workspace_refs: Vec<(String, DepSection)>,
}

/// Key for grouping dependencies that should share a workspace entry
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct WorkspaceDepKey {
    name: String,
    package: Option<String>,
    registry: Option<String>,
}

#[derive(Debug)]
pub(crate) struct DependencyAnalysis {
    /// Dependencies that will be (or are already) consolidated to workspace.dependencies
    /// Includes both newly consolidated deps and resolved version conflicts
    pub(crate) common_deps: Vec<CommonDependency>,

    /// Dependencies with version conflicts that could not be resolved
    pub(crate) conflicts: Vec<ConflictingDependency>,

    /// Workspace dependencies that are not used by any member
    pub(crate) unused_workspace_deps: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CommonDependency {
    pub(crate) name: String,
    pub(crate) version: String,
    /// Members and their sections that use this dependency
    pub(crate) members: Vec<(String, DepSection)>,
    /// Renamed package (e.g., serde_crate = { package = "serde", ... })
    pub(crate) package: Option<String>,
    /// Custom registry for private crates
    pub(crate) registry: Option<String>,
    /// Whether to disable default features
    pub(crate) default_features: bool,
    /// Original version map if this was resolved from a conflict
    pub(crate) resolved_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone)]
pub(crate) struct VersionSpec {
    pub(crate) version: String,
    pub(crate) default_features: bool,
    pub(crate) members: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct VersionUsage {
    /// Members and their sections that use this version
    members: Vec<(String, DepSection)>,
    /// Whether this version is defined in [workspace.dependencies]
    in_workspace: bool,
}

#[derive(Debug, Default)]
struct DependencyTracker {
    /// Maps (version, default_features) -> usage info
    version_specs: HashMap<(String, bool), VersionUsage>,
}

impl DependencyTracker {
    /// Get unique versions
    fn unique_versions(&self) -> std::collections::HashSet<String> {
        self.version_specs.keys().map(|(v, _)| v.clone()).collect()
    }

    /// Count of unique versions
    fn version_count(&self) -> usize {
        self.unique_versions().len()
    }

    /// Check if any version is defined in workspace
    fn has_workspace(&self) -> bool {
        self.version_specs.values().any(|usage| usage.in_workspace)
    }

    /// Get all members across all versions
    fn all_members(&self) -> Vec<(String, DepSection)> {
        self.version_specs
            .values()
            .flat_map(|usage| &usage.members)
            .cloned()
            .collect()
    }

    /// Get all default_features values for a specific version
    fn get_default_features_for_version(&self, version: &str) -> Vec<bool> {
        self.version_specs
            .iter()
            .filter_map(|((v, df), _)| (v == version).then_some(*df))
            .collect()
    }

    /// Build version map for version resolver (groups by version, aggregates members)
    fn build_version_map(&self) -> HashMap<String, Vec<String>> {
        let mut result: HashMap<String, VersionUsage> = HashMap::new();

        for ((version, _df), usage) in &self.version_specs {
            let entry = result.entry(version.clone()).or_default();
            entry.members.extend(usage.members.iter().cloned());
            entry.in_workspace |= usage.in_workspace;
        }

        result
            .into_iter()
            .map(|(version, usage)| (version, usage.to_member_list()))
            .collect()
    }
}

impl VersionUsage {
    fn to_member_list(&self) -> Vec<String> {
        let mut result: Vec<String> = self.members.iter().map(|(name, _)| name.clone()).collect();
        if self.in_workspace {
            result.push(WORKSPACE_MARKER.to_string());
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictType {
    VersionResolution,
    DefaultFeatures,
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictingDependency {
    pub(crate) name: String,
    pub(crate) version_specs: Vec<VersionSpec>,
    pub(crate) conflict_types: Vec<ConflictType>,
}

/// Get common default_features value from a list, returning the unique value if all agree
/// Returns true (the default) if there's disagreement
fn get_common_default_features(values: &[bool]) -> bool {
    if values.is_empty() || values.iter().all(|&v| v == values[0]) {
        values.first().copied().unwrap_or(true)
    } else {
        true
    }
}

macro_rules! extract_fields {
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

/// Extract dependency spec from TOML item
/// Returns DependencySpec or None if should skip
fn extract_dependency_spec(name: &str, item: &Item, section: DepSection) -> Option<DependencySpec> {
    match item {
        Item::Value(val) if val.is_inline_table() => {
            let table = val.as_inline_table()?;
            // Skip path or git dependencies
            if table.contains_key("path") || table.contains_key("git") {
                return None;
            }
            let (version, package, registry, default_features) = extract_fields!(table);
            let version = version?; // Require version for member deps
            Some(DependencySpec {
                name: name.to_string(),
                version,
                section,
                package,
                registry,
                default_features: default_features.unwrap_or(true),
            })
        }
        Item::Value(val) => val.as_str().map(|s| DependencySpec {
            name: name.to_string(),
            version: s.to_string(),
            section,
            package: None,
            registry: None,
            default_features: true,
        }),
        Item::Table(table) => {
            // Skip path or git dependencies
            if table.contains_key("path") || table.contains_key("git") {
                return None;
            }
            let (version, package, registry, default_features) = extract_fields!(table);
            let version = version?; // Require version for member deps
            Some(DependencySpec {
                name: name.to_string(),
                version,
                section,
                package,
                registry,
                default_features: default_features.unwrap_or(true),
            })
        }
        _ => None,
    }
}

/// Check if a dependency item uses workspace inheritance
fn uses_workspace_inheritance(item: &Item) -> bool {
    match item {
        Item::Table(t) => t.contains_key("workspace"),
        Item::Value(val) if val.is_inline_table() => val
            .as_inline_table()
            .map(|t| t.contains_key("workspace"))
            .unwrap_or(false),
        _ => false,
    }
}

/// Process a single dependency section and extract dependency specs
fn process_dependency_section(
    table: &toml_edit::Table,
    section: DepSection,
    deps: &mut Vec<DependencySpec>,
    workspace_refs: &mut Vec<(String, DepSection)>,
) {
    for (name, item) in table.iter() {
        if uses_workspace_inheritance(item) {
            workspace_refs.push((name.to_string(), section));
            continue;
        }

        if let Some(dep_spec) = extract_dependency_spec(name, item, section) {
            deps.push(dep_spec);
        }
    }
}

/// Parse dependencies from a Cargo.toml file
pub(crate) fn parse_dependencies(
    manifest_path: &Path,
    sections: &[DepSection],
) -> Result<ParsedDependencies> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", manifest_path.display()))?;

    let mut deps = Vec::new();
    let mut workspace_refs = Vec::new();

    for section in sections {
        if let Some(Item::Table(table)) = doc.get(section.as_str()) {
            process_dependency_section(table, *section, &mut deps, &mut workspace_refs);
        }
    }

    Ok(ParsedDependencies {
        explicit_deps: deps,
        workspace_refs,
    })
}

/// Parse workspace dependencies from [workspace.dependencies]
pub(crate) fn parse_workspace_dependencies(
    workspace_manifest: &Path,
) -> Result<HashMap<String, DependencySpec>> {
    let content = fs::read_to_string(workspace_manifest)
        .with_context(|| format!("Failed to read {}", workspace_manifest.display()))?;

    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", workspace_manifest.display()))?;

    let mut workspace_deps = HashMap::new();

    if let Some(Item::Table(workspace)) = doc.get("workspace")
        && let Some(Item::Table(deps_table)) = workspace.get("dependencies")
    {
        for (name, item) in deps_table.iter() {
            let (version, package, registry, default_features) = match item {
                Item::Value(val) if val.is_inline_table() => val
                    .as_inline_table()
                    .map_or((None, None, None, None), |table| extract_fields!(table)),
                Item::Value(val) => (val.as_str().map(|s| s.to_string()), None, None, None),
                Item::Table(table) => extract_fields!(table),
                _ => (None, None, None, None),
            };

            if let Some(version) = version {
                workspace_deps.insert(
                    name.to_string(),
                    DependencySpec {
                        name: name.to_string(),
                        version,
                        section: DepSection::Dependencies, // placeholder, actual section comes from member
                        package,
                        registry,
                        default_features: default_features.unwrap_or(true),
                    },
                );
            }
        }
    }

    Ok(workspace_deps)
}

/// Parse all workspace data (workspace deps + member deps)
pub(crate) fn parse_workspace_data(
    workspace_info: &crate::workspace::WorkspaceInfo,
    sections: &[DepSection],
) -> Result<WorkspaceData> {
    let workspace_deps = parse_workspace_dependencies(&workspace_info.root_manifest)?;

    let mut member_deps = HashMap::new();
    let mut all_workspace_refs = Vec::new();

    for member in &workspace_info.members {
        let parsed = parse_dependencies(&member.manifest_path, sections)?;
        if !parsed.explicit_deps.is_empty() {
            member_deps.insert(member.name.clone(), parsed.explicit_deps);
        }
        all_workspace_refs.extend(parsed.workspace_refs);
    }

    Ok(WorkspaceData {
        workspace_deps,
        member_deps,
        workspace_refs: all_workspace_refs,
    })
}

fn should_consolidate(has_workspace: bool, member_count: usize, min_members: usize) -> bool {
    // Consolidate if already in workspace and has any users,
    // or if not in workspace but meets minimum member threshold
    has_workspace && member_count > 0 || member_count >= min_members
}

/// Populate tracker with existing workspace dependencies
fn track_workspace_dependencies(
    trackers: &mut HashMap<WorkspaceDepKey, DependencyTracker>,
    workspace_deps: &HashMap<String, DependencySpec>,
) {
    for (name, ws_dep) in workspace_deps {
        let key = WorkspaceDepKey {
            name: name.clone(),
            package: ws_dep.package.clone(),
            registry: ws_dep.registry.clone(),
        };

        trackers
            .entry(key)
            .or_default()
            .version_specs
            .entry((ws_dep.version.clone(), ws_dep.default_features))
            .or_default()
            .in_workspace = true;
    }
}

/// Populate tracker with member dependency information
fn track_member_dependencies(
    trackers: &mut HashMap<WorkspaceDepKey, DependencyTracker>,
    member_deps: &HashMap<String, Vec<DependencySpec>>,
) {
    for (member_name, deps) in member_deps {
        for dep in deps {
            let key = WorkspaceDepKey {
                name: dep.name.clone(),
                package: dep.package.clone(),
                registry: dep.registry.clone(),
            };

            trackers
                .entry(key)
                .or_default()
                .version_specs
                .entry((dep.version.clone(), dep.default_features))
                .or_default()
                .members
                .push((member_name.clone(), dep.section));
        }
    }
}

/// Process a dependency and resolve to a common version
fn process_dependency(
    key: &WorkspaceDepKey,
    tracker: &DependencyTracker,
    has_workspace: bool,
    all_members: &[(String, DepSection)],
    min_members: usize,
    resolution_strategy: &crate::VersionResolutionStrategy,
) -> Result<Option<CommonDependency>, ConflictingDependency> {
    let mut conflict_types = Vec::new();
    let version_count = tracker.version_count();

    // Try to resolve version
    let version_resolution = if version_count == 1 {
        let version = tracker.unique_versions().into_iter().next().unwrap();
        Some((version, None))
    } else {
        let member_lists_map = tracker.build_version_map();
        match crate::version_resolver::resolve_version_conflict(
            &member_lists_map,
            resolution_strategy,
        ) {
            Ok((version, _)) => Some((version, Some(member_lists_map))),
            Err(_) => {
                conflict_types.push(ConflictType::VersionResolution);
                None
            }
        }
    };

    // Check for default-features conflicts
    let df_values: Vec<bool> = if let Some((ref version, _)) = version_resolution {
        tracker.get_default_features_for_version(version)
    } else {
        tracker.version_specs.keys().map(|(_, df)| *df).collect()
    };

    let unique: std::collections::HashSet<_> = df_values.iter().copied().collect();
    if unique.len() > 1 {
        conflict_types.push(ConflictType::DefaultFeatures);
    }

    // If any conflicts found, return error
    if !conflict_types.is_empty() {
        return Err(create_conflict(key, &tracker.version_specs, conflict_types));
    }

    // Extract resolved version (we know it exists because no conflicts)
    let (resolved_version, resolved_from) = version_resolution.unwrap();
    let common_default_features = get_common_default_features(&df_values);

    // Count unique members (a member may appear multiple times with different sections)
    let unique_member_count = all_members
        .iter()
        .map(|(name, _)| name)
        .collect::<std::collections::HashSet<_>>()
        .len();

    if should_consolidate(has_workspace, unique_member_count, min_members) {
        Ok(Some(CommonDependency {
            name: key.name.clone(),
            version: resolved_version,
            members: all_members.to_vec(),
            package: key.package.clone(),
            registry: key.registry.clone(),
            default_features: common_default_features,
            resolved_from,
        }))
    } else {
        Ok(None)
    }
}

/// Find workspace dependencies that are not used by any member
fn find_unused_workspace_deps(
    common_deps: &[CommonDependency],
    workspace_refs: &[(String, DepSection)],
    workspace_deps: &HashMap<String, DependencySpec>,
) -> Vec<String> {
    let mut used_deps: std::collections::HashSet<String> = std::collections::HashSet::new();

    for common_dep in common_deps {
        used_deps.insert(common_dep.name.clone());
    }

    for (name, _section) in workspace_refs {
        used_deps.insert(name.clone());
    }

    workspace_deps
        .keys()
        .filter(|name| !used_deps.contains(*name))
        .cloned()
        .collect()
}

fn create_conflict(
    key: &WorkspaceDepKey,
    version_spec_map: &HashMap<(String, bool), VersionUsage>,
    conflict_types: Vec<ConflictType>,
) -> ConflictingDependency {
    let version_specs = version_spec_map
        .iter()
        .map(|((version, default_features), usage)| {
            let mut members: Vec<String> =
                usage.members.iter().map(|(name, _)| name.clone()).collect();
            if usage.in_workspace {
                members.push(WORKSPACE_MARKER.to_string());
            }
            VersionSpec {
                version: version.clone(),
                default_features: *default_features,
                members,
            }
        })
        .collect();
    ConflictingDependency {
        name: key.name.clone(),
        version_specs,
        conflict_types,
    }
}

/// Analyze all aspects of workspace dependencies in one pass
pub(crate) fn analyze_workspace(
    data: &WorkspaceData,
    exclude: &[String],
    min_members: usize,
    resolution_strategy: &crate::VersionResolutionStrategy,
) -> Result<DependencyAnalysis> {
    let mut dep_trackers: HashMap<WorkspaceDepKey, DependencyTracker> = HashMap::new();

    track_member_dependencies(&mut dep_trackers, &data.member_deps);
    track_workspace_dependencies(&mut dep_trackers, &data.workspace_deps);

    // Process each tracked dependency
    let mut common_deps = Vec::new();
    let mut conflicts = Vec::new();

    for (key, tracker) in dep_trackers {
        if exclude.contains(&key.name) {
            continue;
        }

        let has_workspace = tracker.has_workspace();
        let all_members = tracker.all_members();

        // Process dependency
        let result = process_dependency(
            &key,
            &tracker,
            has_workspace,
            &all_members,
            min_members,
            resolution_strategy,
        );

        match result {
            Ok(Some(dep)) => common_deps.push(dep),
            Ok(None) => {} // Doesn't meet consolidation conditions
            Err(conflict) => conflicts.push(conflict),
        }
    }

    // Find unused workspace dependencies
    let unused_workspace_deps =
        find_unused_workspace_deps(&common_deps, &data.workspace_refs, &data.workspace_deps);

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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
                default_features: true,
            },
            DependencySpec {
                name: "anyhow".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: true,
            },
            DependencySpec {
                name: "tokio".into(),
                version: "1.0".into(),
                section: DepSection::Dependencies,
                package: None,
                registry: None,
                default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
                default_features: true,
            },
            DependencySpec {
                name: "rstest".into(),
                version: "0.23".into(),
                section: DepSection::DevDependencies,
                package: None,
                registry: None,
                default_features: true,
            },
            DependencySpec {
                name: "cc".into(),
                version: "1.0".into(),
                section: DepSection::BuildDependencies,
                package: None,
                registry: None,
                default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: true,
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
            default_features: false,
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
            default_features: true,
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

        let parsed = parse_dependencies(&manifest_path, &sections)?;

        // Sort both vectors by name for consistent comparison
        let mut deps = parsed.explicit_deps;
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
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to parse TOML"));
    }

    #[test]
    fn test_missing_file() {
        let result = parse_dependencies(
            std::path::Path::new("/nonexistent/path/Cargo.toml"),
            &[DepSection::Dependencies],
        );
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to read"));
    }
}
