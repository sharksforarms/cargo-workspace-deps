mod test_helpers;

use anyhow::Result;
use cargo_workspace_deps::{Config, OutputFormat, VersionResolutionStrategy};
use std::rc::Rc;
use std::cell::RefCell;
use test_helpers::TestWorkspace;

/// Test that text output format produces the expected output
#[test]
fn test_text_output_format() -> Result<()> {
    let workspace = TestWorkspace::new("test_output_comprehensive/before")?;

    let captured = Rc::new(RefCell::new(String::new()));
    let captured_clone = captured.clone();

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: VersionResolutionStrategy::Highest,
        output_format: OutputFormat::Text,
        output_callback: Some(Box::new(move |s| {
            captured_clone.borrow_mut().push_str(s);
        })),
    })?;

    let output = captured.borrow().clone();

    let expected = r#"Found 3 members

Summary:
  3 dependencies to consolidate
  1 version conflicts resolved
  3 conflicts could not resolve
  2 unused workspace dependencies

Will consolidate:
  anyhow = "1.0.80" in: member1, member2, member3
  rstest = "0.23" in: member1, member2, member3
  serde = "1.0" in: member1, member2, member3

Resolved conflicts (using Highest):
  anyhow: 1.0.75, 1.0.78, 1.0.80 → 1.0.80

Could not resolve:
  bindgen (version resolution):
    0.69 (default-features=true) in: member1, member3
    0.70 (default-features=true) in: member2
  lazy_static (version resolution, default-features differ):
    1.4 (default-features=false) in: member3
    1.4 (default-features=true) in: member1
    1.5 (default-features=false) in: member2
  tokio (default-features differ):
    1.0 (default-features=false) in: member1
    1.0 (default-features=true) in: member2, member3

Unused workspace dependencies:
  regex
  tempfile

Updating workspace Cargo.toml...
Consolidated 3 dependencies
"#;

    assert_eq!(
        output,
        expected,
        "\n=== Expected ===\n{}\n=== Got ===\n{}",
        expected,
        output
    );

    Ok(())
}

/// Test that JSON output format produces valid JSON
#[test]
fn test_json_output_format() -> Result<()> {
    let workspace = TestWorkspace::new("test_output_comprehensive/before")?;

    let captured = Rc::new(RefCell::new(String::new()));
    let captured_clone = captured.clone();

    workspace.run(Config {
        fix: true,
        process_dependencies: true,
        process_dev_dependencies: true,
        process_build_dependencies: true,
        workspace_path: Some(workspace.path.clone()),
        exclude: Vec::new(),
        min_members: 2,
        exclude_members: Vec::new(),
        check: false,
        version_resolution_strategy: VersionResolutionStrategy::Highest,
        output_format: OutputFormat::Json,
        output_callback: Some(Box::new(move |s| {
            captured_clone.borrow_mut().push_str(s);
        })),
    })?;

    let output = captured.borrow().clone();

    // Parse and verify the JSON structure
    let mut json: serde_json::Value = serde_json::from_str(&output)?;

    // Replace the dynamic workspace root path with a fixed value for comparison
    json["workspace"]["root"] = serde_json::Value::String(".".to_string());

    let normalized_output = serde_json::to_string_pretty(&json)?;

    let expected = r#"{
  "common_dependencies": [
    {
      "default_features": true,
      "members": [
        "member1",
        "member2",
        "member3"
      ],
      "name": "anyhow",
      "resolved_from": {
        "1.0.75": [
          "member1"
        ],
        "1.0.78": [
          "member3"
        ],
        "1.0.80": [
          "member2"
        ]
      },
      "section": "dependencies",
      "version": "1.0.80"
    },
    {
      "default_features": true,
      "members": [
        "member1",
        "member2",
        "member3"
      ],
      "name": "rstest",
      "section": "dev-dependencies",
      "version": "0.23"
    },
    {
      "default_features": true,
      "members": [
        "member1",
        "member2",
        "member3"
      ],
      "name": "serde",
      "section": "dependencies",
      "version": "1.0"
    }
  ],
  "conflicts": [
    {
      "conflict_types": [
        "version_resolution"
      ],
      "name": "bindgen",
      "section": "build-dependencies",
      "version_specs": [
        {
          "default_features": true,
          "members": [
            "member1",
            "member3"
          ],
          "version": "0.69"
        },
        {
          "default_features": true,
          "members": [
            "member2"
          ],
          "version": "0.70"
        }
      ]
    },
    {
      "conflict_types": [
        "version_resolution",
        "default_features"
      ],
      "name": "lazy_static",
      "section": "dependencies",
      "version_specs": [
        {
          "default_features": false,
          "members": [
            "member3"
          ],
          "version": "1.4"
        },
        {
          "default_features": true,
          "members": [
            "member1"
          ],
          "version": "1.4"
        },
        {
          "default_features": false,
          "members": [
            "member2"
          ],
          "version": "1.5"
        }
      ]
    },
    {
      "conflict_types": [
        "default_features"
      ],
      "name": "tokio",
      "section": "dependencies",
      "version_specs": [
        {
          "default_features": false,
          "members": [
            "member1"
          ],
          "version": "1.0"
        },
        {
          "default_features": true,
          "members": [
            "member2",
            "member3"
          ],
          "version": "1.0"
        }
      ]
    }
  ],
  "summary": {
    "conflicts_resolved": 1,
    "conflicts_unresolved": 3,
    "dependencies_to_consolidate": 3,
    "unused_workspace_deps": 2
  },
  "unused_workspace_dependencies": [
    "regex",
    "tempfile"
  ],
  "version": "1",
  "workspace": {
    "member_count": 3,
    "root": "."
  }
}"#;

    assert_eq!(
        normalized_output,
        expected,
        "\n=== Expected ===\n{}\n=== Got ===\n{}",
        expected,
        normalized_output
    );

    Ok(())
}
