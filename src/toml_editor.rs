use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, InlineTable, Item, Table, value};

use crate::dependency::{CommonDependency, DepSection};

/// Check if a field should be preserved when converting to workspace dependency
fn should_preserve_field(key: &str) -> bool {
    !matches!(key, "version" | "package" | "registry" | "default-features")
}

/// Macro to copy preserved fields from an iterator to an inline table
macro_rules! copy_preserved_fields {
    ($inline:expr, $iter:expr) => {
        for (key, val) in $iter {
            if should_preserve_field(key) {
                $inline.insert(key, val.clone());
            }
        }
    };
}

/// Update a section's dependencies in the workspace table
fn update_section_deps(
    workspace: &mut Table,
    section: DepSection,
    section_deps: &[&CommonDependency],
) {
    let workspace_key = section.as_str();

    if !workspace.contains_key(workspace_key) {
        workspace[workspace_key] = Item::Table(Table::new());
    }

    if let Some(Item::Table(deps_table)) = workspace.get_mut(workspace_key) {
        for dep in section_deps {
            // Only write default-features if false (true is Cargo's default)
            let needs_inline = dep.package.is_some()
                || dep.registry.is_some()
                || dep.default_features == Some(false);

            if needs_inline {
                let mut inline = InlineTable::new();
                inline.insert("version", dep.version.as_str().into());
                if let Some(package) = &dep.package {
                    inline.insert("package", package.as_str().into());
                }
                if let Some(registry) = &dep.registry {
                    inline.insert("registry", registry.as_str().into());
                }
                if dep.default_features == Some(false) {
                    inline.insert("default-features", false.into());
                }
                deps_table.insert(&dep.name, value(inline));
            } else {
                deps_table.insert(&dep.name, value(&dep.version));
            }
        }
    }
}

/// Add or update workspace dependencies in the root Cargo.toml
pub fn update_workspace_dependencies(
    manifest_path: &Path,
    common_deps: &[CommonDependency],
) -> Result<String> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", manifest_path.display()))?;

    if !doc.contains_key("workspace") {
        doc["workspace"] = Item::Table(Table::new());
    }

    let Some(Item::Table(workspace)) = doc.get_mut("workspace") else {
        anyhow::bail!("Failed to get workspace table");
    };

    for section in [
        DepSection::Dependencies,
        DepSection::DevDependencies,
        DepSection::BuildDependencies,
    ] {
        let mut section_deps: Vec<_> = common_deps
            .iter()
            .filter(|d| d.section == section)
            .collect();

        if section_deps.is_empty() {
            continue;
        }

        section_deps.sort_by(|a, b| a.name.cmp(&b.name));
        update_section_deps(workspace, section, &section_deps);
    }

    Ok(doc.to_string())
}

/// Update a member's Cargo.toml to use workspace dependencies
pub fn update_member_dependencies(
    manifest_path: &Path,
    common_deps: &[CommonDependency],
    member_name: &str,
) -> Result<String> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse TOML at {}", manifest_path.display()))?;

    for dep in common_deps {
        if !dep.members.contains(&member_name.to_string()) {
            continue;
        }

        let section_key = dep.section.as_str();

        if let Some(Item::Table(section_table)) = doc.get_mut(section_key)
            && let Some(existing) = section_table.get(&dep.name)
        {
            let mut inline = InlineTable::new();
            inline.insert("workspace", true.into());

            // Preserve fields like features, optional, etc. (version/package/registry/default-features go to workspace)
            match existing {
                Item::Table(table) => {
                    copy_preserved_fields!(inline, table.iter().filter_map(|(k, v)| {
                        if let Item::Value(val) = v {
                            Some((k, val))
                        } else {
                            None
                        }
                    }));
                }
                Item::Value(val) if val.is_inline_table() => {
                    if let Some(table) = val.as_inline_table() {
                        copy_preserved_fields!(inline, table.iter());
                    }
                }
                _ => {}
            }

            section_table[&dep.name] = value(inline);
        }
    }

    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_table_creation() {
        let mut inline = InlineTable::new();
        inline.insert("workspace", true.into());
        let item = value(inline);

        let rendered = format!("{}", item);
        assert!(rendered.contains("workspace"));
        assert!(rendered.contains("true"));
    }
}
