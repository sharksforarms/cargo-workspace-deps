# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/sharksforarms/cargo-workspace-deps/compare/v0.1.1...v0.1.2) - 2026-01-23

### Fixed

- preserve fields on workspace level too
- always parse all sections for workspaces refs
- consolidate to [workspace] only

## [0.1.1](https://github.com/sharksforarms/cargo-workspace-deps/releases/tag/v0.1.1) - 2026-01-03

### Added
- Initial release
- Consolidate shared dependencies to workspace.dependencies
- Support for dependencies, dev-dependencies, and build-dependencies sections
- Version resolution strategies (skip, highest, highest-compatible, lowest, fail)
- Filter options (exclude dependencies, exclude members, min-members)
- Check mode for CI integration
- JSON and text output formats
