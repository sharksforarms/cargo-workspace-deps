use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, InlineTable, Item, Table, value};

use crate::dependency::CommonDependency;

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

/// Update the [workspace.dependencies] table
fn update_workspace_deps_table(workspace: &mut Table, deps: &[&CommonDependency]) {
    const WORKSPACE_DEPS_KEY: &str = "dependencies";

    if !workspace.contains_key(WORKSPACE_DEPS_KEY) {
        workspace[WORKSPACE_DEPS_KEY] = Item::Table(Table::new());
    }

    if let Some(Item::Table(deps_table)) = workspace.get_mut(WORKSPACE_DEPS_KEY) {
        for dep in deps {
            // Collect preserved fields from existing entry (if any)
            let mut preserved_fields: Vec<(String, toml_edit::Value)> = Vec::new();
            if let Some(existing) = deps_table.get(&dep.name) {
                match existing {
                    Item::Table(table) => {
                        for (k, v) in table.iter() {
                            if should_preserve_field(k) {
                                if let Some(val) = v.as_value() {
                                    preserved_fields.push((k.to_string(), val.clone()));
                                }
                            }
                        }
                    }
                    Item::Value(val) if val.is_inline_table() => {
                        if let Some(table) = val.as_inline_table() {
                            for (k, v) in table.iter() {
                                if should_preserve_field(k) {
                                    preserved_fields.push((k.to_string(), v.clone()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Only write default-features if false (true is Cargo's default)
            let needs_inline = dep.package.is_some()
                || dep.registry.is_some()
                || !dep.default_features
                || !preserved_fields.is_empty();

            // Build the value to insert/update
            let new_value = if needs_inline {
                let mut inline = InlineTable::new();
                inline.insert("version", dep.version.as_str().into());
                if let Some(package) = &dep.package {
                    inline.insert("package", package.as_str().into());
                }
                if let Some(registry) = &dep.registry {
                    inline.insert("registry", registry.as_str().into());
                }
                if !dep.default_features {
                    inline.insert("default-features", false.into());
                }
                // Add preserved fields from existing entry
                for (k, v) in preserved_fields {
                    inline.insert(&k, v);
                }
                value(inline)
            } else {
                value(&dep.version)
            };

            // Check if entry already exists
            if let Some(existing_entry) = deps_table.get_mut(&dep.name) {
                *existing_entry = new_value;
            } else {
                deps_table.insert(&dep.name, new_value);
            }
        }
    }
}

/// Add or update workspace dependencies in the root Cargo.toml
pub(crate) fn update_workspace_dependencies(
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

    let mut all_deps: Vec<_> = common_deps.iter().collect();
    all_deps.sort_by(|a, b| a.name.cmp(&b.name));

    if !all_deps.is_empty() {
        update_workspace_deps_table(workspace, &all_deps);
    }

    Ok(doc.to_string())
}

/// Update a member's Cargo.toml to use workspace dependencies
pub(crate) fn update_member_dependencies(
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
        // Find sections this member uses for this dependency
        let member_sections: Vec<_> = dep
            .members
            .iter()
            .filter(|(name, _)| name == member_name)
            .map(|(_, section)| *section)
            .collect();

        if member_sections.is_empty() {
            continue;
        }

        for section in member_sections {
            let section_key = section.as_str();

            if let Some(Item::Table(section_table)) = doc.get_mut(section_key)
                && let Some(existing) = section_table.get(&dep.name)
            {
                let mut inline = InlineTable::new();
                inline.insert("workspace", true.into());

                // Preserve fields like features, optional, etc.
                match existing {
                    Item::Table(table) => {
                        copy_preserved_fields!(
                            inline,
                            table
                                .iter()
                                .filter_map(|(k, v)| v.as_value().map(|val| (k, val)))
                        );
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
    }

    Ok(doc.to_string())
}
