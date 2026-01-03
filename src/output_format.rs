use crate::VersionResolutionStrategy;
use crate::dependency::{ConflictType, DependencyAnalysis};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;

/// Unified output structure that can be serialized to JSON or formatted as text
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Output {
    pub(crate) workspace: WorkspaceInfo,
    pub(crate) summary: Summary,
    pub(crate) common_dependencies: Vec<Dependency>,
    pub(crate) conflicts: Vec<Conflict>,
    pub(crate) unused_workspace_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WorkspaceInfo {
    pub(crate) root: String,
    pub(crate) member_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Summary {
    pub(crate) dependencies_to_consolidate: usize,
    pub(crate) conflicts_resolved: usize,
    pub(crate) conflicts_unresolved: usize,
    pub(crate) unused_workspace_deps: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) section: String,
    pub(crate) members: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) registry: Option<String>,
    pub(crate) default_features: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) resolved_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Conflict {
    pub(crate) name: String,
    pub(crate) section: String,
    pub(crate) version_specs: Vec<VersionSpec>,
    pub(crate) conflict_types: Vec<ConflictType>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VersionSpec {
    pub(crate) version: String,
    pub(crate) default_features: bool,
    pub(crate) members: Vec<String>,
}

impl Output {
    pub(crate) fn new(
        analysis: &DependencyAnalysis,
        workspace_root: &str,
        member_count: usize,
    ) -> Self {
        let resolved_count = analysis
            .common_deps
            .iter()
            .filter(|d| d.resolved_from.is_some())
            .count();

        Output {
            workspace: WorkspaceInfo {
                root: workspace_root.to_string(),
                member_count,
            },
            summary: Summary {
                dependencies_to_consolidate: analysis.common_deps.len(),
                conflicts_resolved: resolved_count,
                conflicts_unresolved: analysis.conflicts.len(),
                unused_workspace_deps: analysis.unused_workspace_deps.len(),
            },
            common_dependencies: analysis
                .common_deps
                .iter()
                .map(|dep| Dependency {
                    name: dep.name.clone(),
                    version: dep.version.clone(),
                    section: dep.section.as_str().to_string(),
                    members: dep.members.clone(),
                    package: dep.package.clone(),
                    registry: dep.registry.clone(),
                    default_features: dep.default_features,
                    resolved_from: dep.resolved_from.clone(),
                })
                .collect(),
            conflicts: analysis
                .conflicts
                .iter()
                .map(|conflict| Conflict {
                    name: conflict.name.clone(),
                    section: conflict.section.as_str().to_string(),
                    version_specs: conflict
                        .version_specs
                        .iter()
                        .map(|spec| VersionSpec {
                            version: spec.version.clone(),
                            default_features: spec.default_features,
                            members: spec.members.clone(),
                        })
                        .collect(),
                    conflict_types: conflict.conflict_types.clone(),
                })
                .collect(),
            unused_workspace_dependencies: analysis.unused_workspace_deps.clone(),
        }
    }

    /// Sort all arrays for deterministic output
    pub(crate) fn sort(&mut self) {
        self.sort_common_dependencies();
        self.sort_conflicts();
        self.unused_workspace_dependencies.sort();
    }

    fn sort_common_dependencies(&mut self) {
        // Sort common_dependencies by name
        self.common_dependencies.sort_by(|a, b| a.name.cmp(&b.name));

        // Sort members arrays within each dependency
        for dep in &mut self.common_dependencies {
            dep.members.sort();

            // Sort members within resolved_from
            if let Some(resolved) = &mut dep.resolved_from {
                for members in resolved.values_mut() {
                    members.sort();
                }
            }
        }
    }

    /// Sort conflicts and their version specs
    fn sort_conflicts(&mut self) {
        // Sort conflicts by name
        self.conflicts.sort_by(|a, b| a.name.cmp(&b.name));

        // Sort version specs and members within conflicts
        for conflict in &mut self.conflicts {
            conflict.version_specs.sort_by(|a, b| {
                a.version
                    .cmp(&b.version)
                    .then_with(|| a.default_features.cmp(&b.default_features))
            });
            for spec in &mut conflict.version_specs {
                spec.members.sort();
            }
        }
    }

    /// Serialize to JSON format
    pub(crate) fn to_json(&self) -> Result<String> {
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize output to JSON")?;
        Ok(format!("{}\n", json))
    }

    /// Format as human-readable text
    pub(crate) fn to_text(&self, resolution_strategy: &VersionResolutionStrategy) -> String {
        let mut output = String::new();

        // Summary
        output.push_str("\nSummary:\n");
        output.push_str(&format!(
            "  {} dependencies to consolidate\n",
            self.summary.dependencies_to_consolidate
        ));
        if self.summary.conflicts_resolved > 0 {
            output.push_str(&format!(
                "  {} version conflicts resolved\n",
                self.summary.conflicts_resolved
            ));
        }
        if self.summary.conflicts_unresolved > 0 {
            output.push_str(&format!(
                "  {} conflicts could not resolve\n",
                self.summary.conflicts_unresolved
            ));
        }
        if self.summary.unused_workspace_deps > 0 {
            output.push_str(&format!(
                "  {} unused workspace dependencies\n",
                self.summary.unused_workspace_deps
            ));
        }
        output.push('\n');

        // Common dependencies
        if !self.common_dependencies.is_empty() {
            output.push_str("Will consolidate:\n");
            for dep in &self.common_dependencies {
                output.push_str(&format!(
                    "  {} = \"{}\" in: {}\n",
                    dep.name,
                    dep.version,
                    dep.members.join(", ")
                ));
            }
            output.push('\n');

            // Resolved conflicts
            let resolved: Vec<_> = self
                .common_dependencies
                .iter()
                .filter(|d| d.resolved_from.is_some())
                .collect();
            if !resolved.is_empty() {
                output.push_str(&format!(
                    "Resolved conflicts (using {:?}):\n",
                    resolution_strategy
                ));
                for dep in &resolved {
                    if let Some(original_versions) = &dep.resolved_from {
                        let mut versions: Vec<_> = original_versions.keys().collect();
                        versions.sort();
                        output.push_str(&format!(
                            "  {}: {} → {}\n",
                            dep.name,
                            versions
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", "),
                            dep.version
                        ));
                    }
                }
                output.push('\n');
            }
        } else {
            output.push_str("No dependencies to consolidate.\n\n");
        }

        // Conflicts
        if !self.conflicts.is_empty() {
            output.push_str("Could not resolve:\n");
            for conflict in &self.conflicts {
                // Build reason string from conflict types
                let reasons: Vec<&str> = conflict
                    .conflict_types
                    .iter()
                    .map(|ct| match ct {
                        ConflictType::VersionResolution => "version resolution",
                        ConflictType::DefaultFeatures => "default-features differ",
                    })
                    .collect();
                let reason = reasons.join(", ");

                output.push_str(&format!("  {} ({}):\n", conflict.name, reason));

                // Check if this conflict involves default-features differences
                let has_default_features_conflict = conflict
                    .conflict_types
                    .contains(&ConflictType::DefaultFeatures);

                for spec in &conflict.version_specs {
                    let version_display = if has_default_features_conflict {
                        // Show default-features explicitly when it's part of the conflict
                        if spec.default_features {
                            format!("{} (default-features=true)", spec.version)
                        } else {
                            format!("{} (default-features=false)", spec.version)
                        }
                    } else if !spec.default_features {
                        format!("{} (default-features=false)", spec.version)
                    } else {
                        spec.version.clone()
                    };
                    if !spec.members.is_empty() {
                        output.push_str(&format!(
                            "    {} in: {}\n",
                            version_display,
                            spec.members.join(", ")
                        ));
                    }
                }
            }
            output.push('\n');
        }

        // Unused workspace dependencies
        if !self.unused_workspace_dependencies.is_empty() {
            output.push_str("Unused workspace dependencies:\n");
            for dep in &self.unused_workspace_dependencies {
                output.push_str(&format!("  {}\n", dep));
            }
            output.push('\n');
        }

        output
    }
}
