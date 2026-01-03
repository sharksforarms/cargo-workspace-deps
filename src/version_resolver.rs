use anyhow::Result;
use semver::{Version, VersionReq};
use std::collections::HashMap;

use crate::VersionResolutionStrategy;

/// Parse a version string leniently, normalizing to valid semver if needed
/// Examples: "1.0" -> "1.0.0", "2" -> "2.0.0"
fn parse_version_lenient(v: &str) -> Option<Version> {
    // Try parsing as-is first
    if let Ok(version) = Version::parse(v) {
        return Some(version);
    }

    // If it fails, try appending components to make it valid semver
    let parts: Vec<&str> = v.split('.').collect();
    let normalized = match parts.len() {
        1 => format!("{}.0.0", v),
        2 => format!("{}.0", v),
        _ => return None,
    };

    Version::parse(&normalized).ok()
}

/// Resolve version conflicts using the specified strategy
/// Returns (resolved_version, all_members) or error if can't resolve
pub(crate) fn resolve_version_conflict(
    version_map: &HashMap<String, Vec<String>>,
    strategy: &VersionResolutionStrategy,
) -> Result<(String, Vec<String>)> {
    let all_members: Vec<String> = version_map.values().flatten().cloned().collect();

    let versions: Vec<String> = version_map.keys().cloned().collect();

    match strategy {
        VersionResolutionStrategy::Skip => {
            anyhow::bail!("Skip strategy")
        }
        VersionResolutionStrategy::Fail => {
            anyhow::bail!("Version conflict detected with fail strategy")
        }
        VersionResolutionStrategy::Highest => resolve_highest(&versions, all_members),
        VersionResolutionStrategy::Lowest => resolve_lowest(&versions, all_members),
        VersionResolutionStrategy::HighestCompatible => {
            resolve_highest_compatible(&versions, all_members)
        }
    }
}

/// Find and return version by order (highest or lowest)
fn resolve_by_order(
    versions: &[String],
    members: Vec<String>,
    take_last: bool,
) -> Result<(String, Vec<String>)> {
    let mut parsed: Vec<Version> = versions
        .iter()
        .filter_map(|v| parse_version_lenient(v))
        .collect();

    anyhow::ensure!(!parsed.is_empty(), "No valid semver versions found");

    parsed.sort();
    let version = if take_last {
        parsed.last()
    } else {
        parsed.first()
    }
    .unwrap();
    Ok((version.to_string(), members))
}

/// Find and return the highest version
fn resolve_highest(versions: &[String], members: Vec<String>) -> Result<(String, Vec<String>)> {
    resolve_by_order(versions, members, true)
}

/// Find and return the lowest version
fn resolve_lowest(versions: &[String], members: Vec<String>) -> Result<(String, Vec<String>)> {
    resolve_by_order(versions, members, false)
}

/// Find highest version that satisfies all requirements
fn resolve_highest_compatible(
    versions: &[String],
    members: Vec<String>,
) -> Result<(String, Vec<String>)> {
    // Parse as requirements (e.g., "1.0" -> "^1.0")
    let mut reqs = Vec::new();
    for v in versions {
        let req = match VersionReq::parse(v) {
            Ok(r) => r,
            Err(_) => {
                // If it's a valid version (with lenient parsing), create a caret req
                if let Some(version) = parse_version_lenient(v) {
                    VersionReq::parse(&format!("^{}", version)).map_err(|e| {
                        anyhow::anyhow!("Failed to parse version requirement: {}", e)
                    })?
                } else {
                    anyhow::bail!("Invalid version: {}", v);
                }
            }
        };
        reqs.push(req);
    }

    let mut candidates: Vec<Version> = versions
        .iter()
        .filter_map(|v| parse_version_lenient(v))
        .collect();

    if candidates.is_empty() {
        anyhow::bail!("No valid semver versions found");
    }

    candidates.sort();
    candidates.reverse();

    for candidate in &candidates {
        if reqs.iter().all(|req| req.matches(candidate)) {
            return Ok((candidate.to_string(), members));
        }
    }

    anyhow::bail!("No version satisfies all requirements")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    // Tests for lenient version parser
    #[rstest]
    #[case::valid_three_component("1.0.0", Some("1.0.0"))]
    #[case::two_component("1.0", Some("1.0.0"))]
    #[case::single_component("2", Some("2.0.0"))]
    #[case::two_component_minor("0.69", Some("0.69.0"))]
    #[case::valid_as_is("1.2.3", Some("1.2.3"))]
    #[case::with_prerelease("1.0.0-alpha", Some("1.0.0-alpha"))]
    #[case::with_build_metadata("1.0.0+build", Some("1.0.0+build"))]
    #[case::prerelease_two_component("1.0-rc1", None)]
    #[case::invalid_string("invalid", None)]
    #[case::too_many_components("1.2.3.4", None)]
    #[case::empty_string("", None)]
    #[case::wildcard("1.0.*", None)]
    #[case::leading_v("v1.0.0", None)]
    fn test_parse_version_lenient(#[case] input: &str, #[case] expected: Option<&str>) {
        let result = parse_version_lenient(input);
        match expected {
            Some(exp) => {
                assert_eq!(result.unwrap().to_string(), exp);
            }
            None => {
                assert!(result.is_none());
            }
        }
    }

    fn make_version_map(versions: &[(&str, Vec<&str>)]) -> HashMap<String, Vec<String>> {
        versions
            .iter()
            .map(|(ver, members)| {
                (
                    ver.to_string(),
                    members.iter().map(|m| m.to_string()).collect(),
                )
            })
            .collect()
    }

    // Tests for Highest strategy
    #[rstest]
    #[case::minor_versions(&[("1.0.0", vec!["m1"]), ("1.1.0", vec!["m2"])], "1.1.0")]
    #[case::two_component_versions(&[("0.69", vec!["m1"]), ("0.70", vec!["m2"])], "0.70.0")]
    #[case::patch_versions(&[("1.0.100", vec!["m1"]), ("1.0.150", vec!["m2"]), ("1.0.120", vec!["m3"])], "1.0.150")]
    #[case::single_component_versions(&[("2", vec!["m1"]), ("3", vec!["m2"])], "3.0.0")]
    #[case::mixed_versions(&[("0.1.0", vec!["m1"]), ("0.2.0", vec!["m2"]), ("0.1.5", vec!["m3"])], "0.2.0")]
    fn test_highest_strategy(#[case] versions: &[(&str, Vec<&str>)], #[case] expected: &str) {
        let version_map = make_version_map(versions);
        let result = resolve_version_conflict(&version_map, &VersionResolutionStrategy::Highest);

        let (version, _) = result.unwrap();
        assert_eq!(version, expected);
    }

    // Tests for Lowest strategy
    #[rstest]
    #[case::minor_versions(&[("1.0.0", vec!["m1"]), ("1.1.0", vec!["m2"])], "1.0.0")]
    #[case::two_component_versions(&[("0.69", vec!["m1"]), ("0.70", vec!["m2"])], "0.69.0")]
    #[case::patch_versions(&[("1.0.100", vec!["m1"]), ("1.0.150", vec!["m2"]), ("1.0.120", vec!["m3"])], "1.0.100")]
    #[case::single_component_versions(&[("2", vec!["m1"]), ("3", vec!["m2"])], "2.0.0")]
    fn test_lowest_strategy(#[case] versions: &[(&str, Vec<&str>)], #[case] expected: &str) {
        let version_map = make_version_map(versions);
        let result = resolve_version_conflict(&version_map, &VersionResolutionStrategy::Lowest);

        let (version, _) = result.unwrap();
        assert_eq!(version, expected);
    }

    // Tests for HighestCompatible strategy
    #[rstest]
    #[case::compatible_patch_versions(&[("1.0.100", vec!["m1"]), ("1.0.150", vec!["m2"]), ("1.0.120", vec!["m3"])], true, "1.0.150", "")]
    #[case::compatible_minor_versions(&[("1.0", vec!["m1"]), ("1.2", vec!["m2"])], true, "1.2.0", "")]
    #[case::incompatible_major_versions(&[("1.0.0", vec!["m1"]), ("2.0.0", vec!["m2"])], false, "", "No version satisfies all requirements")]
    #[case::incompatible_in_zero_x(&[("0.1.0", vec!["m1"]), ("0.2.0", vec!["m2"])], false, "", "No version satisfies all requirements")]
    #[case::mixed_compatible(&[("1.5.0", vec!["m1"]), ("1.6.0", vec!["m2"]), ("1.5.5", vec!["m3"])], true, "1.6.0", "")]
    #[case::zero_zero_x_incompatible(&[("0.0.1", vec!["m1"]), ("0.0.2", vec!["m2"])], false, "", "No version satisfies all requirements")]
    #[case::compatible_with_two_component(&[("1.5", vec!["m1"]), ("1.6.0", vec!["m2"])], true, "1.6.0", "")]
    #[case::single_to_three_component(&[("2", vec!["m1"]), ("2.1.0", vec!["m2"])], true, "2.1.0", "")]
    fn test_highest_compatible_strategy(
        #[case] versions: &[(&str, Vec<&str>)],
        #[case] should_succeed: bool,
        #[case] expected: &str,
        #[case] expected_err: &str,
    ) {
        let version_map = make_version_map(versions);
        let result =
            resolve_version_conflict(&version_map, &VersionResolutionStrategy::HighestCompatible);

        if should_succeed {
            let (version, _) = result.unwrap();
            assert_eq!(version, expected);
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.to_string(), expected_err);
        }
    }

    // Tests for Skip and Fail strategies
    #[rstest]
    #[case::skip_with_conflict(VersionResolutionStrategy::Skip, "Skip strategy")]
    #[case::fail_with_conflict(
        VersionResolutionStrategy::Fail,
        "Version conflict detected with fail strategy"
    )]
    fn test_no_resolution_strategies(
        #[case] strategy: VersionResolutionStrategy,
        #[case] expected_err: &str,
    ) {
        let version_map = make_version_map(&[("1.0.0", vec!["m1"]), ("1.1.0", vec!["m2"])]);
        let result = resolve_version_conflict(&version_map, &strategy);
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), expected_err);
    }

    // Test that all strategies handle members correctly
    #[rstest]
    #[case::highest(VersionResolutionStrategy::Highest, "1.5.0")]
    #[case::lowest(VersionResolutionStrategy::Lowest, "1.0.0")]
    #[case::highest_compatible(VersionResolutionStrategy::HighestCompatible, "1.5.0")]
    fn test_members_aggregation(
        #[case] strategy: VersionResolutionStrategy,
        #[case] expected_version: &str,
    ) {
        let version_map = make_version_map(&[
            ("1.0.0", vec!["member1", "member2"]),
            ("1.5.0", vec!["member3"]),
        ]);
        let result = resolve_version_conflict(&version_map, &strategy);
        let (version, mut members) = result.unwrap();
        assert_eq!(version, expected_version);
        members.sort();
        assert_eq!(members, vec!["member1", "member2", "member3"]);
    }

    // Test edge cases for all resolution strategies
    #[rstest]
    #[case::highest_empty_map(VersionResolutionStrategy::Highest)]
    #[case::lowest_empty_map(VersionResolutionStrategy::Lowest)]
    #[case::highest_compatible_empty_map(VersionResolutionStrategy::HighestCompatible)]
    fn test_empty_version_map_fails(#[case] strategy: VersionResolutionStrategy) {
        let version_map: HashMap<String, Vec<String>> = HashMap::new();
        let result = resolve_version_conflict(&version_map, &strategy);
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "No valid semver versions found");
    }

    // Test with only invalid versions
    #[rstest]
    #[case::highest_invalid(VersionResolutionStrategy::Highest, "No valid semver versions found")]
    #[case::lowest_invalid(VersionResolutionStrategy::Lowest, "No valid semver versions found")]
    #[case::highest_compatible_invalid(
        VersionResolutionStrategy::HighestCompatible,
        "Invalid version:"
    )]
    fn test_invalid_versions_fail(
        #[case] strategy: VersionResolutionStrategy,
        #[case] expected_err: &str,
    ) {
        let version_map = make_version_map(&[("invalid1", vec!["m1"]), ("invalid2", vec!["m2"])]);
        let result = resolve_version_conflict(&version_map, &strategy);
        let err = result.unwrap_err();
        assert!(err.to_string().starts_with(expected_err));
    }

    // Test edge cases specific to HighestCompatible
    #[rstest]
    #[case::zero_zero_x_incompatible(&[("0.0.1", vec!["m1"]), ("0.0.2", vec!["m2"])], false, "", "No version satisfies all requirements")]
    #[case::prerelease_versions(&[("1.0.0-alpha", vec!["m1"]), ("1.0.0-beta", vec!["m2"])], true, "1.0.0-beta", "")]
    #[case::zero_x_patch_compatible(&[("0.1.0", vec!["m1"]), ("0.1.5", vec!["m2"])], true, "0.1.5", "")]
    fn test_highest_compatible_edge_cases(
        #[case] versions: &[(&str, Vec<&str>)],
        #[case] should_succeed: bool,
        #[case] expected: &str,
        #[case] expected_err: &str,
    ) {
        let version_map = make_version_map(versions);
        let result =
            resolve_version_conflict(&version_map, &VersionResolutionStrategy::HighestCompatible);

        if should_succeed {
            let (version, _) = result.unwrap();
            assert_eq!(version, expected);
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.to_string(), expected_err);
        }
    }
}
