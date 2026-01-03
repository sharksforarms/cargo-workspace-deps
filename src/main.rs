use anyhow::Result;
use cargo_workspace_deps::{CheckFailure, Config, OutputFormat, VersionResolutionStrategy, run};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cargo-workspace-deps")]
#[command(bin_name = "cargo")]
#[command(about = "Consolidate common dependencies into workspace.dependencies")]
#[command(
    long_about = "Moves shared dependencies to [workspace.dependencies] and updates members to use workspace = true.
Reduces duplication and ensures version consistency across the workspace."
)]
enum Cargo {
    #[command(name = "workspace-deps")]
    #[command(about = "Consolidate common dependencies into workspace.dependencies")]
    WorkspaceDeps(Args),
}

#[derive(Parser, Debug)]
#[command(version, about)]
#[command(
    long_about = "Moves shared dependencies to [workspace.dependencies] and updates members to use workspace = true.
Reduces duplication and ensures version consistency across the workspace."
)]
struct Args {
    /// Apply changes without prompting for confirmation
    #[arg(long)]
    fix: bool,

    /// Check mode: exit with error if changes needed (useful for CI)
    #[arg(long)]
    check: bool,

    /// Path to workspace directory (defaults to current directory)
    #[arg(long, value_name = "PATH")]
    manifest_path: Option<std::path::PathBuf>,

    /// Skip processing [dependencies] section
    #[arg(long)]
    no_dependencies: bool,

    /// Skip processing [dev-dependencies] section
    #[arg(long)]
    no_dev_dependencies: bool,

    /// Skip processing [build-dependencies] section
    #[arg(long)]
    no_build_dependencies: bool,

    /// Skip specific dependencies by name (comma-separated, e.g. serde,tokio)
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,

    /// Skip workspace members by glob pattern (comma-separated, e.g. submodules/*,deps/*)
    #[arg(long, value_delimiter = ',', value_parser = parse_glob_pattern)]
    exclude_members: Vec<glob::Pattern>,

    /// Only consolidate dependencies appearing in at least N members
    #[arg(long, default_value = "2")]
    min_members: usize,

    /// Strategy for resolving version conflicts
    #[arg(long, value_enum, default_value = "highest-compatible")]
    version_resolution: VersionResolutionStrategy,

    /// Output format
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
}

fn parse_glob_pattern(s: &str) -> Result<glob::Pattern, String> {
    glob::Pattern::new(s).map_err(|e| format!("Invalid glob pattern '{}': {}", s, e))
}

fn main() -> Result<()> {
    let Cargo::WorkspaceDeps(args) = Cargo::parse();

    // JSON output for non-interactive paths only
    if args.format == OutputFormat::Json && !args.fix && !args.check {
        anyhow::bail!("JSON output requires --fix or --check flag (non-interactive mode)");
    }

    let config = Config {
        fix: args.fix,
        process_dependencies: !args.no_dependencies,
        process_dev_dependencies: !args.no_dev_dependencies,
        process_build_dependencies: !args.no_build_dependencies,
        workspace_path: args.manifest_path,
        exclude: args.exclude,
        min_members: args.min_members,
        exclude_members: args.exclude_members,
        check: args.check,
        version_resolution_strategy: args.version_resolution,
        output_format: args.format,
        output_callback: None,
    };

    match run(config) {
        Ok(()) => Ok(()),
        Err(e) if e.downcast_ref::<CheckFailure>().is_some() => {
            std::process::exit(1);
        }
        Err(e) => Err(e),
    }
}
