use anyhow::Result;
use semver::{Version, VersionReq};
use std::collections::HashMap;

use crate::VersionResolutionStrategy;

/// Resolve version conflicts using the specified strategy
/// Returns (resolved_version, all_members) or error if can't resolve
pub fn resolve_version_conflict(
    version_map: &HashMap<String, Vec<String>>,
    strategy: &VersionResolutionStrategy,
) -> Result<(String, Vec<String>)> {
    let all_members: Vec<String> = version_map.values().flatten().cloned().collect();

    let versions: Vec<String> = version_map
        .keys()
        .map(|spec| {
            spec.split('|')
                .next()
                .unwrap_or(spec)
                .trim_start_matches("version=")
                .to_string()
        })
        .collect();

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
        .filter_map(|v| Version::parse(v).ok())
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
                // If it's a valid version, create a caret req
                if let Ok(version) = Version::parse(v) {
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
        .filter_map(|v| Version::parse(v).ok())
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

    fn make_version_map(versions: &[(&str, Vec<&str>)]) -> HashMap<String, Vec<String>> {
        versions
            .iter()
            .map(|(ver, members)| {
                (
                    format!("version={}", ver),
                    members.iter().map(|m| m.to_string()).collect(),
                )
            })
            .collect()
    }

    #[rstest]
    #[case(VersionResolutionStrategy::Highest, "1.0.150")]
    #[case(VersionResolutionStrategy::Lowest, "1.0.100")]
    fn test_version_selection_strategies(
        #[case] strategy: VersionResolutionStrategy,
        #[case] expected_version: &str,
    ) {
        let version_map = make_version_map(&[
            ("1.0.100", vec!["member1"]),
            ("1.0.150", vec!["member2"]),
            ("1.0.120", vec!["member3"]),
        ]);

        let result = resolve_version_conflict(&version_map, &strategy);
        assert!(result.is_ok());

        let (version, members) = result.unwrap();
        assert_eq!(version, expected_version);
        assert_eq!(members.len(), 3);
    }

    #[rstest]
    #[case(VersionResolutionStrategy::Skip)]
    #[case(VersionResolutionStrategy::Fail)]
    fn test_failing_strategies(#[case] strategy: VersionResolutionStrategy) {
        let version_map =
            make_version_map(&[("1.0.0", vec!["member1"]), ("1.1.0", vec!["member2"])]);

        let result = resolve_version_conflict(&version_map, &strategy);
        assert!(result.is_err());
    }

    #[rstest]
    #[case(
        &[
            ("1.0.100", vec!["member1"]),
            ("1.0.150", vec!["member2"]),
            ("1.0.120", vec!["member3"]),
        ],
        true,
        "1.0.150"
    )]
    #[case(
        &[
            ("1.0.0", vec!["member1"]),
            ("2.0.0", vec!["member2"]),
        ],
        false,
        ""
    )]
    fn test_highest_compatible(
        #[case] versions: &[(&str, Vec<&str>)],
        #[case] should_succeed: bool,
        #[case] expected_version: &str,
    ) {
        let version_map = make_version_map(versions);

        let result =
            resolve_version_conflict(&version_map, &VersionResolutionStrategy::HighestCompatible);

        if should_succeed {
            assert!(result.is_ok());
            let (version, _) = result.unwrap();
            assert_eq!(version, expected_version);
        } else {
            assert!(result.is_err());
        }
    }
}
