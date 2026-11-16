# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.1] - 2026-11-16

### Changed
- Dependency upgrades to the latest versions: rmcp 3.9.0 -> 3.10.0, rand 0.10.3 -> 0.10.4, jsonschema 0.61.0 -> 0.61.2, plus transitive lockfile updates (rpassword 7.5.5, cc, mio, rustls-webpki, crc32fast, and others). No source changes were required.

### Fixed
- `ado-cli get-work-item --comments` and `ado-cli comments` failed with a `404` on any on-prem server without `ADO_DEFAULT_PROJECT`. The comments API requires the project in the URL; the client now uses the configured default project, or reads `System.TeamProject` from the work item, and names `ADO_DEFAULT_PROJECT` when neither is available.
- `ado-cli get-work-item --expand Comments` resolved the wrong project, so it could still ask for an endpoint without a project segment. It now uses the project from the work item it already fetched.
- `ado-cli add-comment` on an on-prem server never reached the discussion thread. The comments API accepts only `api-version=6.0-preview`, the SDK sent `6.0`, and every failure was swallowed into a `System.History` field patch. The client now posts to the thread directly.
- `ado-cli add-comment` fell back to `System.History` even when the cause was a missing project, hiding the one error the user can fix. Only an unreachable comments API now reaches the fallback; a missing project is reported with the variable to set.
- A comment could be posted twice when a write failed in a way that did not prove the server had rejected it. The client now retries a write only after an authentication rejection, and treats an unreadable success response as success.
- The `ado-cli search` examples in the README and the shipped skills used `--project`, which the command does not accept. The project is the first positional argument. A new test runs every documented command line through the CLI's parser, so a documented flag that does not exist now fails the build.

## [0.7.0] - 2026-11-14

### Added
- `ado-cli skills` serves built-in agent usage guides from inside the binary, so the guidance always matches the installed CLI version. `ado-cli skills list`, `ado-cli skills get <name>`, `ado-cli skills get <name> --full`, `ado-cli skills get <name> --ref <path>`, and `ado-cli skills path`. Two skills ship: `core` (workflow and command surface) and `on-prem` (Azure DevOps Server quirks). The command needs no token, no config file, and no network.
- `ADO_CLI_SKILLS_DIR` replaces the bundled skills with a directory of your own, for iterating on skill content without a rebuild.
- `ado-cli skills` is CLI-only, with no MCP tool, like `login` and `logout`.

### Changed
- Documentation refreshed across `README.md`, `CONTEXT.md`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/SPEC.md`, `docs/RELEASE.md`, and the ADRs. The specification's requirements are renumbered to remove a duplicate `FR-5`, and the architecture section now describes the crates and modules as they exist.

### Fixed
- `ado-cli skills path` ended without a trailing newline, which ran its output into the next shell prompt.

## [0.6.0] - 2026-11-09

### Changed
- Dependency upgrades to latest available versions: rmcp 3.5.1 -> 3.9.0, tokio 1.53.3 -> 1.54.1, htmd 0.5.5 -> 0.5.6, jsonschema 0.58.6 -> 0.61.0 (dev/test dependency; pre-1.0 bump), plus transitive lockfile updates (pulldown-cmark, syn, uuid, async-compression, and others). No source changes were required; tests, clippy and fmt all green.

## [0.5.0] - 2026-11-09

### Security
- Config file hardening: a group/world-writable `config.yaml` is refused at load; new config directories are created with mode `0700`; an existing config file is tightened to `0600` through the open handle before new secrets are written.
- Endpoint URLs containing a username/password (userinfo) are rejected when the config is loaded.
- `ADO_CA_CERT` now applies the custom CA to this program's own HTTP clients instead of mutating the global `SSL_CERT_FILE` environment variable.
- Human-readable CLI output escapes terminal control characters (ESC, BEL, CR, ...) in text coming from Azure DevOps, so a malicious work item field can no longer spoof the terminal.

### Fixed
- `update_work_item` now actually enforces `expected_rev`: it sends a JSON Patch `test` op on `/rev`, so a revision mismatch returns `Conflict` instead of silently overwriting concurrent edits.
- Work item IDs outside the SDK's 32-bit range return a clear error instead of being silently truncated, which could previously operate on the wrong work item.
- `manage_work_item_link` removals are scoped to the requested `link_type` (default `System.LinkTypes.Related`): a Hyperlink whose URL shares the numeric suffix of the target is no longer removed by mistake.

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
