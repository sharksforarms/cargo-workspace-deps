#!/bin/bash
set -e

if [ $# -lt 1 ]; then
    echo "Usage: $0 <test_name> [member_count]"
    echo "Example: $0 version_conflicts 2"
    exit 1
fi

TEST_NAME=$1
MEMBER_COUNT=${2:-2}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures/test_$TEST_NAME"

echo "Creating test: test_$TEST_NAME"
echo "  Fixture directory: $FIXTURES_DIR"
echo "  Members: $MEMBER_COUNT"

# Create fixture directories
mkdir -p "$FIXTURES_DIR"/{before,after}

# Create member directories with src/
for ((i=1; i<=MEMBER_COUNT; i++)); do
    mkdir -p "$FIXTURES_DIR/before/member$i/src"
    mkdir -p "$FIXTURES_DIR/after/member$i/src"

    # Create lib.rs
    echo "// lib" > "$FIXTURES_DIR/before/member$i/src/lib.rs"
    echo "// lib" > "$FIXTURES_DIR/after/member$i/src/lib.rs"
done

# Generate member list for workspace Cargo.toml
MEMBERS=""
for ((i=1; i<=MEMBER_COUNT; i++)); do
    if [ $i -gt 1 ]; then
        MEMBERS+=", "
    fi
    MEMBERS+="\"member$i\""
done

# Create workspace Cargo.toml templates
cat > "$FIXTURES_DIR/before/Cargo.toml" <<EOF
[workspace]
members = [$MEMBERS]
resolver = "2"
EOF

cat > "$FIXTURES_DIR/after/Cargo.toml" <<EOF
[workspace]
members = [$MEMBERS]
resolver = "2"

[workspace.dependencies]
# TODO: Add expected workspace dependencies here
EOF

# Create member Cargo.toml templates
for ((i=1; i<=MEMBER_COUNT; i++)); do
    cat > "$FIXTURES_DIR/before/member$i/Cargo.toml" <<EOF
[package]
name = "member$i"
version = "0.1.0"
edition = "2021"

[dependencies]
# TODO: Add dependencies here
EOF

    cat > "$FIXTURES_DIR/after/member$i/Cargo.toml" <<EOF
[package]
name = "member$i"
version = "0.1.0"
edition = "2021"

[dependencies]
# TODO: Update with expected result
EOF
done

# Create test file if it doesn't exist
TEST_FILE="$SCRIPT_DIR/test_$TEST_NAME.rs"
if [ ! -f "$TEST_FILE" ]; then
    cat > "$TEST_FILE" <<EOF
mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::Config;
use test_helpers::TestWorkspace;

#[test]
fn test_description_here() -> Result<()> {
    let workspace = TestWorkspace::new("test_$TEST_NAME/before")?;

    workspace.run(Config {
        dry_run: false,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        include_submodules: false,
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
    })?;

    workspace.assert_matches("test_$TEST_NAME/after")?;

    Ok(())
}
EOF
    echo "  Created test file: tests/test_$TEST_NAME.rs"
else
    echo "  Test file already exists: tests/test_$TEST_NAME.rs"
fi

echo "Next steps:"
echo "  1. Edit fixtures in: tests/fixtures/test_$TEST_NAME/"
echo "     - before/: Set up initial workspace state"
echo "     - after/: Define expected result"
echo "  2. Update test file: tests/test_$TEST_NAME.rs"
echo "  3. Run test: cargo test --test test_$TEST_NAME"
