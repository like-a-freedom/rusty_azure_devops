# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.2] - 2026-10-14

### Fixed
- Release builds no longer fail with `cannot stat target/.../release/rusty_azure_devops`: the package ships two binaries (`ado-mcp`, `ado-cli`) and the workflow now publishes both for every platform.
- The Docker image build no longer fails at `cargo fetch`: the build stage creates placeholder targets before fetching dependencies.

## [0.4.1] - 2026-10-14

### Fixed
- Release automation now actually runs: `.github` is no longer gitignored, so a release publishes the multi-platform binaries and the multi-arch Docker image.
- Codecov upload no longer blocks the release build when no token is configured.

### Changed
- CI runs only on release (and manual dispatch) — no workflows on push or pull requests.
- GitHub Actions updated to latest: actions/checkout v7, actions/cache v6, actions-rust-lang/setup-rust-toolchain v2, codecov/codecov-action v7, docker/* v4/v7, softprops/action-gh-release v3.
- Dependabot version-update pull requests disabled (`open-pull-requests-limit: 0`).

## [0.4.0] - 2026-10-14

### Added
- `ado-cli login` / `ado-cli logout`: first-run authentication that saves the PAT and server URL to `~/.config/rusty_azure_devops/config.yaml` (mode 0600), validating credentials via `list_projects` before saving. Prompts only on a TTY; headless runs use `--base-url` with `--token`, `--token-stdin`, or `ADO_TOKEN` and otherwise fail fast.
- Agent-friendly login contract: `--token-stdin` keeps the PAT out of argv and shell history; `--json` shapes both success and failure (errors go to stderr as `{"ok":false,"reason","detail","hint"}` with empty stdout); stable reason tokens; exit codes 0 ok / 2 usage / 3 needs `--force` / 1 other; success carries a `next` verify command.

### Changed
- Dependency upgrades to latest available versions: rmcp 3.5.1, jsonschema 0.58.6, schemars 1.2.3, thiserror 2.0.21, serde_json 1.0.154, tokio 1.53.3, reqwest 0.13.5, rand 0.10.3 (plus transitive lockfile updates; all tests green).
- Toolchain pinned to `stable` via `rust-toolchain.toml` (MSRV policy tracks stable); removed the fixed component list.

### Fixed
- `ado-cli login` no longer panics on a malformed `--base-url`; it returns a `validation_failed` error envelope (exit 1) instead of crashing with exit 101.
- Migrated to rmcp `ServerConfig` (the `ServerInfo` alias is deprecated).

## [0.3.0] - 2026-09-04

### Changed
- Dependency upgrades to latest available versions: azure_devops_rust_api 0.39.1, base64 0.23.1, jsonschema 0.53.0, rmcp 3.2.0, tokio 1.53.1, serde 1.0.229, schemars 1.2.2, thiserror 2.0.20, serde_json 1.0.151, async-trait 0.1.92, htmd 0.5.5, serde_yaml 0.9.34, pulldown-cmark 0.13.4 (all 207 tests green, no source changes required)

## [0.2.1] - 2026-09-04

### Fixed
- `get-query-results` now includes `workItemRelations` IDs, so tree/oneHop saved queries no longer return an empty list
- `get-query-results` accepts `/_queries/query-edit/GUID` saved-query URLs in addition to `/_queries/query/GUID`
- Empty `get-query-results` output tells whether the query returned nothing or the page offset skipped past the total

## [0.2.0] - 2026-08-26

### Added
- 10 MCP tools covering work item, project, query, and comment operations
- Dual interface: stdio MCP server (`ado-mcp`) + standalone CLI (`ado-cli`) sharing one business-logic layer
- On-prem TFS support via `DowngradeApiVersionPolicy` and `OnPremFallback` (direct HTTP)
- Custom CA certificate support for self-signed / corporate servers (`ADO_CA_CERT`)
- Retry strategy with exponential backoff and jitter (env-driven `RT_RETRY_MAX` / `RT_BASE_DELAY_MS`)
- Multi-platform release builds (Linux x86_64, macOS x86_64, macOS aarch64, Windows x86_64)
- Distroless multi-arch Docker image (amd64 + arm64)
- 19 JSON schemas for tool contracts under `docs/schemas/`
- 15 Architecture Decision Records under `docs/adr/`
- CI coverage via `cargo-llvm-cov`, uploaded to Codecov

### Notes
- First public release.
