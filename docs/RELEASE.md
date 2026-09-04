# Release Guide

## Versioning

- **Workspace crates:** `azure_devops_client` and `rusty_azure_devops` version in lockstep
- **Pre-1.0:** Minor bumps (0.1.0 → 0.2.0) for new features or breaking dependency upgrades
- **Post-1.0:** Follow semver strictly

## Release Steps

1. **Update version** in `[workspace.package]` in the root `Cargo.toml` (both crates inherit it via `version.workspace = true`)
2. **Run quality gates:**
   ```bash
   cargo check --workspace
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --workspace
   ```
3. **Commit:** `release: bump version to X.Y.Z`
4. **Tag:** `git tag vX.Y.Z`
5. **Push:** `git push && git push --tags`
6. **Update CHANGELOG.md** with release notes

## Release Notes Format

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...
```
