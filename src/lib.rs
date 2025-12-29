pub mod dependency;
pub mod output_format;
pub mod toml_editor;
pub mod version_resolver;
pub mod workspace;

use anyhow::{Context, Result};
use dependency::{DepSection, analyze_workspace, parse_workspace_data};
use toml_editor::{update_member_dependencies, update_workspace_dependencies};
use workspace::discover_workspace;

#[derive(Clone, Debug)]
pub enum VersionResolutionStrategy {
    Skip,
    Highest,
    HighestCompatible,
    Lowest,
    Fail,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

pub struct Config {
    pub fix: bool,
    pub process_dependencies: bool,
    pub process_dev_dependencies: bool,
    pub process_build_dependencies: bool,
    pub workspace_path: Option<std::path::PathBuf>,
    pub exclude: Vec<String>,
    pub min_members: usize,
    pub exclude_members: Vec<String>,
    pub check: bool,
    pub version_resolution_strategy: VersionResolutionStrategy,
    pub output_format: OutputFormat,
    #[allow(clippy::type_complexity)]
    pub output_callback: Option<Box<dyn Fn(&str)>>,
}

/// Helper function to write output (either to callback or stdout)
fn write_output(config: &Config, text: &str) {
    if let Some(callback) = &config.output_callback {
        callback(text);
    } else {
        print!("{}", text);
    }
}

/// Main entry point for the workspace dependency consolidation
pub fn run(config: Config) -> Result<()> {
    let mut workspace = discover_workspace(config.workspace_path.as_deref())?;
    let filtered_patterns = workspace.filter_by_patterns(&config.exclude_members);

    // Print workspace info only for text output
    if config.output_format == OutputFormat::Text {
        if filtered_patterns > 0 {
            write_output(&config, &format!(
                "Found {} members ({} excluded by pattern)\n",
                workspace.members.len(),
                filtered_patterns
            ));
        } else {
            write_output(&config, &format!("Found {} members\n", workspace.members.len()));
        }
    }

    let sections: Vec<_> = [
        (config.process_dependencies, DepSection::Dependencies),
        (config.process_dev_dependencies, DepSection::DevDependencies),
        (
            config.process_build_dependencies,
            DepSection::BuildDependencies,
        ),
    ]
    .iter()
    .filter_map(|(enabled, section)| enabled.then_some(*section))
    .collect();

    if sections.is_empty() {
        if config.output_format == OutputFormat::Text {
            write_output(&config, "No dependency sections selected for processing.\n");
        }
        return Ok(());
    }

    let workspace_data = parse_workspace_data(&workspace, &sections)?;
    let analysis = analyze_workspace(
        &workspace_data,
        &config.exclude,
        config.min_members,
        &config.version_resolution_strategy,
    )?;

    // Create unified output structure
    let workspace_root = workspace
        .root_manifest
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");
    let mut output_data = output_format::Output::new(&analysis, workspace_root, workspace.members.len());
    output_data.sort();

    // Output analysis based on format
    match config.output_format {
        OutputFormat::Text => {
            write_output(&config, &output_data.to_text(&config.version_resolution_strategy));
        }
        OutputFormat::Json => {
            // JSON output handled in check mode or before prompt
        }
    }

    // Check mode: return error if there are dependencies to consolidate
    if config.check {
        // Output JSON if requested
        if config.output_format == OutputFormat::Json {
            write_output(&config, &output_data.to_json()?);
        }

        if !analysis.common_deps.is_empty() {
            if config.output_format == OutputFormat::Text {
                write_output(&config, &format!(
                    "Check failed: {} dependencies could be consolidated\n",
                    analysis.common_deps.len()
                ));
            }
            anyhow::bail!("Check failed: dependencies could be consolidated");
        } else if !analysis.conflicts.is_empty() {
            if config.output_format == OutputFormat::Text {
                write_output(&config, &format!(
                    "Check failed: {} unresolved conflicts\n",
                    analysis.conflicts.len()
                ));
            }
            anyhow::bail!("Check failed: unresolved conflicts");
        } else {
            if config.output_format == OutputFormat::Text {
                write_output(&config, "Check passed: no dependencies to consolidate\n");
            }
            return Ok(());
        }
    }

    if analysis.common_deps.is_empty() {
        if config.output_format == OutputFormat::Json {
            write_output(&config, &output_data.to_json()?);
        }
        return Ok(());
    }

    // For JSON output, require --fix flag (non-interactive)
    if config.output_format == OutputFormat::Json && !config.fix {
        anyhow::bail!("JSON output requires --fix flag (non-interactive mode)");
    }

    // Prompt for confirmation unless --fix is used
    if !config.fix {
        write_output(&config, "Apply these changes? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        let answer = input.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            write_output(&config, "Cancelled.\n");
            return Ok(());
        }
        write_output(&config, "\n");
    }

    if config.output_format == OutputFormat::Text {
        write_output(&config, "Updating workspace Cargo.toml...\n");
    }

    let workspace_content =
        update_workspace_dependencies(&workspace.root_manifest, &analysis.common_deps)?;
    std::fs::write(&workspace.root_manifest, &workspace_content)
        .with_context(|| format!("Failed to write {}", workspace.root_manifest.display()))?;

    for member in &workspace.members {
        let member_content =
            update_member_dependencies(&member.manifest_path, &analysis.common_deps, &member.name)?;

        let original = std::fs::read_to_string(&member.manifest_path)?;
        if original != member_content {
            std::fs::write(&member.manifest_path, &member_content)
                .with_context(|| format!("Failed to write {}", member.manifest_path.display()))?;
        }
    }

    // Output final summary
    if config.output_format == OutputFormat::Text {
        write_output(&config, &output_format::format_summary(analysis.common_deps.len()));
    } else {
        write_output(&config, &output_data.to_json()?);
    }

    Ok(())
}
