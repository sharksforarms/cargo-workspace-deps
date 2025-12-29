use anyhow::{Context, Result};
use crate::VersionResolutionStrategy;
use crate::dependency::{DependencyAnalysis, ConflictType};
use serde::Serialize;
use std::collections::HashMap;

/// Unified output structure that can be serialized to JSON or formatted as text
#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub version: String,
    pub workspace: WorkspaceInfo,
    pub summary: Summary,
    pub common_dependencies: Vec<Dependency>,
    pub conflicts: Vec<Conflict>,
    pub unused_workspace_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceInfo {
    pub root: String,
    pub member_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub dependencies_to_consolidate: usize,
    pub conflicts_resolved: usize,
    pub conflicts_unresolved: usize,
    pub unused_workspace_deps: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub section: String,
    pub members: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub name: String,
    pub section: String,
    pub version_specs: Vec<VersionSpec>,
    pub conflict_types: Vec<ConflictType>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionSpec {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    pub members: Vec<String>,
}

impl Output {
    /// Create a new Output from DependencyAnalysis
    pub fn new(
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
            version: "1".to_string(),
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
    pub fn sort(&mut self) {
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

        // Sort conflicts by name
        self.conflicts.sort_by(|a, b| a.name.cmp(&b.name));

        // Sort version specs and members within conflicts
        for conflict in &mut self.conflicts {
            conflict.version_specs.sort_by(|a, b| {
                match a.version.cmp(&b.version) {
                    std::cmp::Ordering::Equal => a.default_features.cmp(&b.default_features),
                    other => other,
                }
            });
            for spec in &mut conflict.version_specs {
                spec.members.sort();
            }
        }

        // Sort unused workspace dependencies
        self.unused_workspace_dependencies.sort();
    }

    /// Serialize to JSON format
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize output to JSON")?;
        Ok(format!("{}\n", json))
    }

    /// Format as human-readable text
    pub fn to_text(&self, resolution_strategy: &VersionResolutionStrategy) -> String {
        let mut output = String::new();

        // Summary header
        output.push_str("\nSummary:\n");
        output.push_str(&format!(
            "  {} dependencies to consolidate\n",
            self.summary.dependencies_to_consolidate
        ));
        if self.summary.conflicts_resolved > 0 {
            output.push_str(&format!("  {} version conflicts resolved\n", self.summary.conflicts_resolved));
        }
        if self.summary.conflicts_unresolved > 0 {
            output.push_str(&format!("  {} conflicts could not resolve\n", self.summary.conflicts_unresolved));
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
                output.push_str(&format!("Resolved conflicts (using {:?}):\n", resolution_strategy));
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
                let reasons: Vec<&str> = conflict.conflict_types.iter().map(|ct| match ct {
                    ConflictType::VersionResolution => "version resolution",
                    ConflictType::DefaultFeatures => "default-features differ",
                }).collect();
                let reason = reasons.join(", ");

                output.push_str(&format!("  {} ({}):\n", conflict.name, reason));

                for spec in &conflict.version_specs {
                    let version_display = match spec.default_features {
                        Some(false) => format!("{} (default-features=false)", spec.version),
                        Some(true) => format!("{} (default-features=true)", spec.version),
                        None => spec.version.clone(),
                    };
                    if !spec.members.is_empty() {
                        output.push_str(&format!("    {} in: {}\n", version_display, spec.members.join(", ")));
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

/// Format a simple completion summary
pub fn format_summary(common_deps_count: usize) -> String {
    format!("Consolidated {} dependencies\n", common_deps_count)
}
