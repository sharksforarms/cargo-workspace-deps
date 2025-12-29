use anyhow::Result;
use cargo_workspace_deps::{Config, run};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cargo-workspace-deps")]
#[command(bin_name = "cargo")]
#[command(about = "Consolidate common dependencies into workspace.dependencies")]
#[command(long_about = "Moves shared dependencies to [workspace.dependencies] and updates members to use workspace = true.
Reduces duplication and ensures version consistency across the workspace.")]
enum Cargo {
    #[command(name = "workspace-deps")]
    #[command(about = "Consolidate common dependencies into workspace.dependencies")]
    WorkspaceDeps(Args),
}

#[derive(Parser, Debug)]
#[command(version, about)]
#[command(long_about = "Moves shared dependencies to [workspace.dependencies] and updates members to use workspace = true.
Reduces duplication and ensures version consistency across the workspace.")]
#[command(after_help = "EXAMPLES:
    cargo workspace-deps                    # Preview changes
    cargo workspace-deps --fix              # Apply changes
    cargo workspace-deps --check            # CI mode: error if changes needed
    cargo workspace-deps --exclude tokio    # Exclude specific deps
    cargo workspace-deps --min-members 3    # Require 3+ members sharing dep")]
struct Args {
    /// Apply changes without prompting for confirmation
    #[arg(long)]
    fix: bool,

    /// Check mode: exit with error if changes needed (for CI)
    #[arg(long)]
    check: bool,

    /// Process [dependencies] section (use --no-dependencies to disable)
    #[arg(long, default_value = "true")]
    dependencies: bool,

    /// Process [dev-dependencies] section (use --no-dev-dependencies to disable)
    #[arg(long, default_value = "true")]
    dev_dependencies: bool,

    /// Process [build-dependencies] section (use --no-build-dependencies to disable)
    #[arg(long, default_value = "true")]
    build_dependencies: bool,

    /// Dependencies to skip consolidation (by name, e.g. serde,tokio)
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,

    /// Members to skip analysis (by glob, e.g. examples/*,benches/*)
    #[arg(long, value_delimiter = ',')]
    exclude_members: Vec<String>,

    /// Only consolidate dependencies appearing in at least N members
    #[arg(long, default_value = "2")]
    min_members: usize,

    /// Strategy for resolving version conflicts
    #[arg(long, value_enum, default_value = "highest-compatible")]
    version_resolution: VersionResolutionStrategy,

    /// Output format (text or json)
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum VersionResolutionStrategy {
    /// Skip dependencies with version conflicts
    Skip,
    /// Use highest version found
    Highest,
    /// Use highest SemVer-compatible version (default)
    HighestCompatible,
    /// Use lowest version found
    Lowest,
    /// Exit with error on conflicts
    Fail,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

impl From<VersionResolutionStrategy> for cargo_workspace_deps::VersionResolutionStrategy {
    fn from(strategy: VersionResolutionStrategy) -> Self {
        match strategy {
            VersionResolutionStrategy::Skip => Self::Skip,
            VersionResolutionStrategy::Highest => Self::Highest,
            VersionResolutionStrategy::HighestCompatible => Self::HighestCompatible,
            VersionResolutionStrategy::Lowest => Self::Lowest,
            VersionResolutionStrategy::Fail => Self::Fail,
        }
    }
}

impl From<OutputFormat> for cargo_workspace_deps::OutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Text => Self::Text,
            OutputFormat::Json => Self::Json,
        }
    }
}

fn main() -> Result<()> {
    let Cargo::WorkspaceDeps(args) = Cargo::parse();

    let config = Config {
        fix: args.fix,
        process_dependencies: args.dependencies,
        process_dev_dependencies: args.dev_dependencies,
        process_build_dependencies: args.build_dependencies,
        workspace_path: None, // Run in current directory
        exclude: args.exclude,
        min_members: args.min_members,
        exclude_members: args.exclude_members,
        check: args.check,
        version_resolution_strategy: args.version_resolution.into(),
        output_format: args.format.into(),
        output_callback: None,
    };

    run(config).inspect_err(|e| {
        if e.to_string().contains("Check failed") {
            std::process::exit(1);
        }
    })
}
