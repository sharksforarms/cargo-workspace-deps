# cargo-workspace-deps

[![crates.io](https://img.shields.io/crates/v/cargo-workspace-deps.svg)](https://crates.io/crates/cargo-workspace-deps)

A Cargo subcommand that consolidates shared dependencies across workspace members into `[workspace.dependencies]`.

## What it does

Moves common dependencies to the workspace-level `[workspace.dependencies]` section and updates member crates to use `workspace = true`.
This reduces duplication and ensures version consistency across your Cargo workspace.

## Installation

### `cargo install` ([crates.io](https://crates.io/crates/cargo-workspace-deps))

```
cargo install cargo-workspace-deps --locked
```

### `cargo install` ([`master`](https://github.com/sharksforarms/cargo-workspace-deps/tree/master))

```
cargo install --git https://github.com/sharksforarms/cargo-workspace-deps --locked
```

## Examples

```bash
# Interactive mode (prompts for confirmation)
cargo workspace-deps

# Automatically apply changes
cargo workspace-deps --fix

# Check only, useful for CI
cargo workspace-deps --check

# Only consolidate dependencies used by 3+ members (default is 2)
cargo workspace-deps --min-members 3

# Skip specific dependencies
cargo workspace-deps --exclude "serde,tokio" --exclude-members "submodules/*"
```

## Usage

```bash
$ cargo workspace-deps --help

Moves shared dependencies to [workspace.dependencies] and updates members to use workspace = true.
Reduces duplication and ensures version consistency across the workspace.

Usage: cargo workspace-deps [OPTIONS]

Options:
      --fix
          Apply changes without prompting for confirmation

      --check
          Check mode: exit with error if changes needed (useful for CI)

      --manifest-path <PATH>
          Path to workspace directory (defaults to current directory)

      --no-dependencies
          Skip processing [dependencies] section

      --no-dev-dependencies
          Skip processing [dev-dependencies] section

      --no-build-dependencies
          Skip processing [build-dependencies] section

      --exclude <EXCLUDE>
          Skip specific dependencies by name (comma-separated, e.g. serde,tokio)

      --exclude-members <EXCLUDE_MEMBERS>
          Skip workspace members by glob pattern (comma-separated, e.g. submodules/*,deps/*)

      --min-members <MIN_MEMBERS>
          Only consolidate dependencies appearing in at least N members

          [default: 2]

      --version-resolution <VERSION_RESOLUTION>
          Strategy for resolving version conflicts

          Possible values:
          - skip:               Skip dependencies with conflicting versions
          - highest:            Use the highest version
          - highest-compatible: Use the highest SemVer-compatible version
          - lowest:             Use the lowest version
          - fail:               Fail on version conflicts

          [default: highest-compatible]

      --format <FORMAT>
          Output format

          [default: text]
          [possible values: text, json]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Limitations

Path dependencies (`path = "..."`), git dependencies (`git = "..."`), and platform-specific dependencies (`[target.'cfg(...)'.dependencies]`) are currently not supported and will be automatically skipped.

