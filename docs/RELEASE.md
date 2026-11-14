# Release Guide

## Versioning

- **Workspace crates:** `azure_devops_client` and `rusty_azure_devops` version in lockstep.
- **Pre-1.0:** a minor bump (0.6.0 to 0.7.0) for a new feature or a breaking dependency upgrade; a patch bump for fixes.
- **Post-1.0:** semver strictly.

## Release steps

1. **Update the version** in `[workspace.package]` in the root `Cargo.toml`. Both crates inherit it through `version.workspace = true`.
2. **Run the quality gates:**
   ```bash
   cargo check --workspace
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --workspace
   ```
3. **Update `CHANGELOG.md`** with the release notes (see the format below).
4. **Commit:** `chore: release vX.Y.Z`.
5. **Tag** the release commit: `git tag vX.Y.Z`.
6. **Push** the commit and the tag: `git push && git push --tags`.
7. **Publish the GitHub release** from the tag:
   ```bash
   gh release create vX.Y.Z --title "vX.Y.Z" --notes-file CHANGELOG-section.md
   ```

The tag must point at the released commit. Push the commit before the tag so a
failed push never leaves a tag pointing at something nobody can fetch.

## Release notes format

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...
```

Keep a Changelog order: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`,
`Security`. Leave out a section with no entries. Write each entry from the
reader's side, in plain English, and name the command or flag it affects.

## What counts as which bump

| Change | Bump |
|---|---|
| A new command, flag, or tool | Minor |
| A changed exit code or JSON shape | Minor, and call it out in the notes |
| A removed or renamed command, flag, or tool | Minor before 1.0, with the rename spelled out |
| A bug fix with no interface change | Patch |
| A security fix | Patch, under `Security` |
| Documentation, refactors, CI | No release |

The `ado-cli skills` content is part of the binary, so a change to a skill
changes the release. Mention it in the notes under `Added` or `Changed`.
