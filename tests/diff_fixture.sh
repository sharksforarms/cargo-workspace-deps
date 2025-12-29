#!/bin/bash
set -e

if [ $# -lt 1 ]; then
    echo "Usage: $0 <test_name>"
    echo "Example: $0 default"
    echo ""
    echo "Available tests:"
    cd "$(dirname "${BASH_SOURCE[0]}")/fixtures" && ls -d test_* | sed 's/test_/  - /'
    exit 1
fi

TEST_NAME=$1
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BEFORE_DIR="$SCRIPT_DIR/fixtures/test_$TEST_NAME/before"
AFTER_DIR="$SCRIPT_DIR/fixtures/test_$TEST_NAME/after"

if [ ! -d "$BEFORE_DIR" ]; then
    echo "Error: Test fixture not found: test_$TEST_NAME"
    echo ""
    echo "Available tests:"
    cd "$SCRIPT_DIR/fixtures" && ls -d test_* | sed 's/test_/  - /'
    exit 1
fi

echo "=== Diff for test_$TEST_NAME ==="
echo ""
echo "Comparing:"
echo "  before: $BEFORE_DIR"
echo "  after:  $AFTER_DIR"
echo ""

# Find all Cargo.toml files and diff them
find "$BEFORE_DIR" -name "Cargo.toml" -type f | sort | while read -r before_file; do
    rel_path="${before_file#$BEFORE_DIR/}"
    after_file="$AFTER_DIR/$rel_path"

    if [ ! -f "$after_file" ]; then
        echo "⚠️File only in before: $rel_path"
        continue
    fi

    if ! diff -q "$before_file" "$after_file" > /dev/null 2>&1; then
        echo "📝 $rel_path:"
        echo "----------------------------------------"
        git diff --no-index --color=always "$before_file" "$after_file" | tail -n +5 || true
        echo ""
    fi
done

# Check for files only in after
find "$AFTER_DIR" -name "Cargo.toml" -type f | sort | while read -r after_file; do
    rel_path="${after_file#$AFTER_DIR/}"
    before_file="$BEFORE_DIR/$rel_path"

    if [ ! -f "$before_file" ]; then
        echo "⚠️File only in after: $rel_path"
    fi
done
