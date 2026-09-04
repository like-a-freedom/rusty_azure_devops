# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
