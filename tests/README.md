# Integration Tests

## Test Structure

Each test file (`test_*.rs`) is its own integration test module. Related tests are grouped together in the same file. Each test uses fixtures from `fixtures/`:

```
tests/
├── test_helpers.rs       # Shared test utilities
├── test_default.rs       # Default behavior tests
├── test_check.rs         # --check mode tests (multiple tests)
├── test_exclude.rs       # --exclude flag tests
└── fixtures/
    ├── test_default/
    │   ├── before/       # Initial workspace state
    │   └── after/        # Expected result after running tool
    └── ...
```

## Adding a New Test

### Quick Start: Use the create_test.sh script

The easiest way to create a new test:

```bash
cd tests
./create_test.sh my_feature 2
```

This creates:
- `tests/test_my_feature.rs` - Test file template
- `tests/fixtures/test_my_feature/before/` - Initial workspace (2 members)
- `tests/fixtures/test_my_feature/after/` - Expected result
- All necessary subdirectories and boilerplate files

Then:
1. Edit the fixtures to set up your test case
2. Update the test function in `test_my_feature.rs`
3. Run: `cargo test --test test_my_feature`

### Option 1: Add to existing test file

If your test relates to an existing feature:

```rust
// In tests/test_check.rs

#[test]
fn my_new_check_test() -> Result<()> {
    let workspace = TestWorkspace::new("test_check_my_case/before")?;
    // ... test code ...
}
```

Then manually create fixtures:
```bash
./create_test.sh check_my_case 2
```

### Option 2: Manual Setup

If you prefer not to use the script:

1. **Create fixture directories**:
   ```bash
   mkdir -p tests/fixtures/test_my_feature/{before,after}/{member1,member2}/src
   ```

2. **Create Cargo.toml and lib.rs files** in each directory

3. **Create test file** `tests/test_my_feature.rs`

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test test_check

# Run specific test function
cargo test --test test_check fails_when_consolidation_possible
```

## Test Helper API

- `TestWorkspace::new(fixture_path)` - Creates temp workspace from fixture
- `workspace.run(config)` - Runs tool with given config
- `workspace.assert_matches(expected)` - Compares result with expected fixture
- `workspace.path` - PathBuf to temp workspace root

## Helper Scripts

### create_test.sh - Create new test scaffolding

```bash
Usage: ./create_test.sh <test_name> [member_count]

Arguments:
  test_name      Name of the test (will create test_<name>.rs)
  member_count   Number of workspace members (default: 2)

Examples:
  ./create_test.sh version_conflicts        # 2 members
  ./create_test.sh large_workspace 5        # 5 members
```

The script creates:
- Test file with boilerplate
- Fixture directories (before/after)
- Workspace and member Cargo.toml templates
- src/lib.rs files for each member
- All with TODO comments for easy editing

### diff_fixture.sh - View expected changes

```bash
Usage: ./diff_fixture.sh <test_name>

Examples:
  ./diff_fixture.sh default
  ./diff_fixture.sh renamed_deps
```

Shows a colored diff of what changes are expected in the test fixture (before → after)

## Tips

- Use `./create_test.sh` to quickly scaffold new tests
- Use `./diff_fixture.sh <test>` to see what changes a test expects
- Group related tests in the same file (e.g., all `--check` tests in `test_check.rs`)
- Each test is isolated in its own temp directory
- Tests can run in parallel safely
- Use simple, minimal fixtures that test one thing
- Name test functions descriptively: `does_something`, `fails_when_xyz`
