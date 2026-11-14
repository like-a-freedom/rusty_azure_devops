# SRS — rusty_azure_devops

**Version:** 0.7
**Date:** 2026-11-14

## 1. Project overview

**Name:** rusty_azure_devops

**Purpose:** An MCP (Model Context Protocol) server in Rust (stable) that works with an on-premises Azure DevOps Server through the REST API. It exposes a set of tools over stdio (rmcp) for basic Work Item (WI) operations: create, read (fields, links, comments), update, and search.

**Constraints:**
- Authentication is mandatory. The server refuses to start without `ADO_TOKEN` and `ADO_BASE_URL` (FR-9).
- No attachments in the first release.
- No local persistence or cache is required.
- Works with any project the API token has access to.

## 2. Goals and scope

- Provide a convenient, extensible MCP interface for Azure DevOps Work Item operations.
- Support integration with external automations and LLM agents over stdio (rmcp).
- Focus on correctness of operations (atomicity, concurrency control via rev/test), clear errors, and predictable behavior.

## 3. Terminology and definitions

- WI — a Work Item in Azure DevOps (Bug, Task, User Story, Custom, etc.).
- MCP — Model Context Protocol (rmcp implementation).
- rmcp — the Rust crate for implementing MCP (supports stdio transport, tools, schema validation, etc.).
- azure_devops_client — the library crate holding the `AzureDevOpsClient` trait, the `azure_devops_rust_api` adapter, and the resilience seams.
- Skill — an agent-facing usage guide served by `ado-cli skills` (FR-10).

## 3.1 Configuration: environment variables (required)

**All** configuration parameters are set exclusively through environment variables (ENV). No MCP command changes or saves configuration between runs; the config is read from the environment at startup.

Required and recommended environment variables:

- `ADO_TOKEN` (required) — the Personal Access Token (PAT) for accessing Azure DevOps; the server does not start without it.
- `ADO_BASE_URL` (required) — the base URL of the Azure DevOps on-prem or cloud instance (for example `https://dev.azure.com/org` or `https://ado-server/collection`). HTTPS is mandatory for remote servers; HTTP is allowed only for loopback addresses of local services and tests. `ADO_BASE_URL` must be set together with `ADO_TOKEN`.
- `ADO_ORGANIZATION` (optional) — the organization (used together with `ADO_BASE_URL` when needed).
- `ADO_API_VERSION` (optional) — the REST API version; the default is **latest** (when supported by the client library) and can be overridden via ENV.
- `ADO_DEFAULT_PROJECT` (optional) — if set, used as the default project for commands where `project` is not passed.
- `ADO_CA_CERT` (optional) — path to a PEM/DER custom CA for TLS connections to the on-prem server; the certificate is added to the Azure SDK HTTP clients and the fallback separately, without changing `SSL_CERT_FILE`.
- `ADO_CLI_SKILLS_DIR` (optional) — replaces the bundled agent skills with a directory of your own. An empty value is treated as unset; a missing directory is an error rather than a silent fallback.
- `RUST_LOG` — the log level (`info`, `warn`, `debug`, `trace`).

Note: configuration changes only by updating environment variables and restarting the service.

Update 2026-11: the primary human and agent path is `ado-cli login` (FR-9), and `ado-cli skills` (FR-10) needs no configuration at all. The ENV-only statement above applies to the MCP server runtime.

## 4. Functional requirements (FR)

Each requirement includes a short description and acceptance criteria.

FR-1: Create Work Item
- Description: the `create_work_item` tool takes `project`, `type` (any WI type), required fields (at least `System.Title`), and optional fields/relations/comments.
- Implementation: an `application/json-patch+json` document is built and sent to POST `/workitems/{type}?api-version=<version>`.
- Acceptance: returns an object with the new `id`, `url`, and the field set confirmed by the Azure server.

FR-2: Get Work Item
- Description: `get_work_item` returns a WI by `id` (or a list of `ids`) with optional `fields` and `$expand` (Relations/Links/All/Comments) parameters.
- Implementation: GET `/workitems/{id}?fields=...&$expand=...`.
- Acceptance: the fields are returned, plus relations and comments when `$expand` is requested.

FR-3: Update Work Item
- Description: `update_work_item` takes a JSON Patch (add/replace/remove/test operations, etc.) and an optional `expected_rev` parameter (a number). When `expected_rev` is set, the agent includes a `test` op on `/rev` in the JSON Patch for optimistic concurrency; a mismatch returns a `Conflict` error (409). When `expected_rev` is omitted, the PATCH runs without a preceding test, like a plain PATCH; passing `expected_rev` is recommended to avoid lost updates.
- Implementation: PATCH `/workitems/{id}?api-version=...` with Content-Type `application/json-patch+json` and, when `expected_rev` is present, `test { "op":"test","path":"/rev","value":<expected_rev> }` as the first operation.
- Acceptance: on success, the updated WI with an incremented revision is returned; on a rev mismatch, a `Conflict` with an explanation and the current revision.

FR-4: Search Work Items
- Description: `search_work_items` supports WIQL and simplified filters; it returns a list of ids; `details=true` triggers a batch GET `/workitems?ids=...` for full records.
- Implementation: POST `/wit/wiql` → collect ids → batch GET `/workitems?ids=...` (details).
- Constraints: pagination is not implemented in the first release. A `limit=100` applies by default (overridable by a request parameter). When the number of matched ids exceeds the practical usage threshold, the client should issue additional queries manually.
- Acceptance: a WIQL query returns the expected ids (capped at `limit`); with `details=true` the fields for the returned ids are returned correctly.

FR-5: Manage Work Item Links
- Description: `manage_work_item_link` adds and removes links between work items (link types: `System.LinkTypes.Related`, `System.LinkTypes.Hierarchy-Forward`/`Hierarchy-Reverse`, `System.LinkTypes.Dependency-Forward`, etc.). It supports `add` and `remove`.
- Input parameters (MVP): `action` ("add" | "remove"), `source_id` (the id of the work item being changed), `target` (the id of the target work item), `link_type` (optional, defaults to `System.LinkTypes.Related`), `attributes` (optional).
- Implementation: for `add`, a JSON Patch `add` op is built on `/relations/-` with `{ "rel": <link_type>, "url": <target_url>, "attributes": ... }` and `update_work_item` is called; for `remove`, `get_work_item` finds the index of the existing relation and a `remove` op targets `/relations/{index}`.
- Acceptance:
  - A successful `add` returns the updated list of relations in the result.
  - A successful `remove` deletes the relation and returns the updated list of relations.
  - Negative case: removing a nonexistent relation returns a clear `relation not found` error.
  - Input and output schemas live in `docs/schemas/manage_work_item_link.schema.json` and `manage_work_item_link_result.schema.json`.

FR-6: Management commands and project discovery
- Description: configuration is set only via ENV (see section 3.1); MCP exposes only tools for reading/inspecting configuration and dynamically discovering resources.
- Commands:
  - `get_status` — returns the current configuration (without secrets) and the connection state to Azure DevOps.
  - `list_projects` — queries and returns the list of projects available to the credentials in `ADO_TOKEN`.
- Default project: when `project` is not passed to a tool, MCP applies this priority order: 1) `ADO_DEFAULT_PROJECT` (if set), 2) if exactly one project is available for the token, that project is used, 3) otherwise the command returns an error requiring an explicit `project`.
- Note: `set_instance` / `set_token` commands are **not supported**; changing configuration requires updating ENV and restarting the service.

FR-7: StdIO / rmcp contract
- Description: every command is registered as an rmcp tool with explicit JSON input/output schemas (schemars).
- Acceptance: schemas validate, and client compatibility is preserved.

FR-8: Errors and retry
- Transient errors (timeouts/5xx): retry with exponential backoff (max 3 attempts).
- Client errors (4xx): do not retry; return a structured error with the code and message.

FR-9: Primary authorization (login)
- Description: the `ado-cli login` CLI command saves the PAT and the server URL to the user config `~/.config/rusty_azure_devops/config.yaml` with 0600 permissions. Values come from flags (`--base-url`, `--token`, `--token-stdin`), then environment (`ADO_BASE_URL`, `ADO_TOKEN`), then interactive prompts (TTY only, PAT input hidden). Before saving, a verification through `list_projects` runs (disabled with `--no-verify`). URL scheme validation always applies: remote HTTP URLs are not saved even with `--no-verify`.
- Implementation: `ado-cli login [--base-url --token|--token-stdin --organization --project --api-version --ca-cert --force --no-verify --json]`, `ado-cli logout [--json]`.
- Acceptance: without a TTY and without flags or env values, the command exits quickly with `missing_value` (exit code 1) instead of blocking on input; overwriting an existing config requires `--force` (otherwise `confirmation_required`, exit code 3); with `--json`, errors go to stderr as `{"ok":false,"reason","detail","hint"}` and stdout stays empty; the token is never printed in any mode.

FR-10: Built-in agent skills
- Description: `ado-cli skills` serves agent-facing usage guides embedded in the binary, so they always match the installed CLI version. Subcommands: `list`, `get <name> [--full] [--ref <path>]`, `path`. CLI-only, with no MCP tool (ADR-0017), like `login`/`logout`.
- Implementation: skill files live in `crates/rusty_azure_devops/skills/<name>/SKILL.md` with optional `references/`, embedded at compile time via `include_dir!`. `ADO_CLI_SKILLS_DIR` replaces the bundled catalog for local iteration; an empty value is treated as unset, and a missing directory is an error rather than a silent fallback. The command is dispatched before credential loading.
- Acceptance: `ado-cli skills get core` prints the skill markdown byte-exactly and works with no `ADO_TOKEN`, no config file, and no network; `ado-cli skills list --json` returns `{"ok":true,"skills":[…]}`; runtime errors go to stderr as `{"ok":false,"reason","detail","hint"}` with stable reasons `skill_not_found`, `reference_not_found`, `validation_failed`, `io_error` and exit code 1, stdout empty on failure; clap usage errors remain plain text with exit code 2 even under `--json`; a test guards that the skill documents every CLI command and no other.

## 5. Non-functional requirements (NFR)

NFR-1: Performance
- Targets: P50 < 300ms for simple GET/CREATE; P95 < 1s under normal LAN conditions.

NFR-2: Scalability
- Expected load: roughly 0-100 RPS. The architecture should allow scaling the HTTP client if needed.

NFR-3: Reliability
- Availability target: 99.5% in a production environment under normal load.

NFR-4: Extensibility
- The `AzureDevOpsClient` interface is a Rust trait; new backend drivers can be added without changing `rusty_azure_devops`.

NFR-5: Observability and logging
- Structured logging through `tracing`, with levels `info`, `warn`, `debug`, `trace`. The format is JSON-compatible with fields `timestamp`, `level`, `target`, `message`, `context`.
- Metrics: Prometheus/metrics are not required in the first release; additional metrics can be added later on request.
- Tracing: OpenTelemetry is optional and not required for the MVP. Logs must not contain secrets (API tokens, keys, credentials).

NFR-6: Security
- Authentication is mandatory: the PAT must be provided through the `ADO_TOKEN` and `ADO_BASE_URL` environment variables (see section 3.1). Without valid values for these variables, the service does not start.
- Secrets must not leak into diagnostic errors or logs. Fallback errors contain only the API version, the HTTP status, or a generic reason; the PAT and the server response body are not included.
- Provide guidance on minimal PAT scopes and token rotation.
- Config protection: a `config.yaml` file (next to the binary or in `~/.config/rusty_azure_devops/`) that is group/world-writable (`0o022`) is not read; the command fails. New config directories are created with mode `0700`; existing directories are not modified. An existing config file is tightened to `0600` through the open handle before new secrets are written.
- The endpoint URL must be HTTPS (HTTP is allowed only for loopback); URLs with a username/password (userinfo) are rejected when the config is loaded.
- In human-readable CLI output, control characters (ESC, BEL, CR, etc.) in text coming from Azure DevOps are escaped into visible `\u{XX}` form; newlines and tabs are preserved. JSON output is produced by the serializer (structured).
- The CI pipeline uses least privilege: the workflow level is `contents: read`; write is needed only by the jobs that publish release artifacts (`contents: write`) and the Docker image (`packages: write`).

## 6. Architecture and design

### Components
1. `rusty_azure_devops` — the rmcp-based server and the CLI. It registers tools, handles the stdio transport through `rmcp`, validates schemas, routes tools, and owns the CLI command surface including `ado-cli skills`.
2. `azure_devops_client` — the `AzureDevOpsClient` trait, the `azure_devops_rust_api` adapter, and the two resilience seams `RetryStrategy` (backoff with jitter) and `OnPremFallback` (the direct HTTP GET that bypasses the SDK for on-prem quirks).
3. `operations/` — the shared service layer. `WorkItemOperations`, `LinkOperations`, `CommentOperations`, `ProjectLookup`, and `QueryOperations` hold the business logic; both the MCP handler and the CLI call them.
4. `domain/` — the DTOs, parameter structs, Patch Document construction, and the constants.
5. `config` — the config loader (ENV, `config.yaml`, and CLI overrides) and the `ADO_DEFAULT_PROJECT` selection rule.
6. `skills/` — the agent usage guides embedded in the binary and served by `ado-cli skills`.

### Dataflow
Client → stdio/rmcp → `rusty_azure_devops` handler → `operations/` → `AzureDevOpsClient` → Azure DevOps → back again as JSON.

The CLI follows the same path, from clap arguments to the same operations, so the two interfaces cannot diverge.

### Design decisions
- Use the `rmcp` crate for the stdio transport and the server logic (details below, under "Notes on `rmcp`").
- `AzureDevOpsClient` as a trait for easily testable mock/fake implementations.
- Retry/backoff and the on-prem fallback are two separate seams in `azure_devops_client::resilience`.
- The `api-version` is a parameterizable configuration setting.
- The skill content is embedded at compile time, so it is rebuilt and re-versioned with the CLI it describes.

### Crates in use
- rmcp (MCP protocol, schema validation, stdio transport)
- azure_devops_rust_api (the Azure DevOps client, `wit` and `core` features)
- tokio (async runtime)
- serde / serde_json (serialization)
- schemars (JSON schema generation for tool contracts)
- serde_yaml (config and skill frontmatter)
- clap (CLI argument parsing)
- include_dir (embedding the skill content at compile time)
- tracing (structured logging)
- thiserror (errors)
- wiremock (HTTP-level tests)

## 7. Notes on `rmcp` (refreshed via Context7)

Key points (from Context7 `/websites/rs-rmcp`):
- `rmcp` provides ready examples for building services and transports over stdio (stdin/stdout) and TCP.
- It supports tool registration and automatic schema validation (schemars) for inputs and outputs.
- It ships with transport implementations (stdio, TCP, SSE client) and convenient macros/methods for working with a peer (list_tools, call_tool, etc.).
- For a stdio server, prefer `serve_directly`/`serve_directly_with_ct` and the Transport from Standard IO examples.

Link: https://docs.rs/rmcp/latest/

## 8. StdIO tools (Tool Contracts): schemas (descriptive)

Each tool must have an input and output JSON schema (schemars) and be described in `docs/schemas/`.

Examples (descriptive):

`create_work_item` — input:
```json
{ "project": "string", "type": "string", "fields": {"System.Title": "...", "Custom.Foo":"..."}, "relations": [...], "comments": [ { "author": "...", "text": "..." } ] }
```
output:
```json
{ "id": 123, "url": "https://...", "fields": {...} }
```

Comments: MCP reads and writes comments through the Work Item Comments API (the preferred way). Behavior:
- In `create_work_item`, when `comments` are provided, MCP first creates the WI, then calls the Comments API for each comment (POST `/wit/workItems/{id}/comments`).
- `get_work_item` with `$expand=Comments` returns comments from the Comments API (including author, text, and timestamp).
- For simplicity and backward compatibility, MCP also supports writing short comments through JSON Patch (appending to the `System.History` field), but the Comments API is recommended.

`get_work_item` — input:
```json
{ "id": 123, "project": "optional", "fields": ["System.Title","System.AssignedTo"], "expand": "All|Relations|None" }
```
output: the WorkItem record (id, rev, fields, relations, _links, comments)

`update_work_item` — input:
```json
{ "id": 123, "patch": [ {"op":"add","path":"/fields/System.Tags","value":"foo"} ] }
```
output: the updated WorkItem

`search_work_items` — input:
```json
{ "project": "optional", "wiql": "SELECT [System.Id] FROM WorkItems WHERE ...", "details": true }
```
output: `{ "ids": [1,2,3], "total": 3 }` (with details=true, the detailed records are added)

## 9. Error handling and retry policy (details)

- Transient errors (timeouts, network errors, 5xx): retry with exponential backoff and jitter (for example 100ms, 300ms, 900ms), max 3 attempts; log each attempt and the final outcome.
- Rate limiting (429): when Azure returns 429 with a `Retry-After` header, MCP must honor `Retry-After` and repeat the request after the indicated delay, applying the general retry logic with backoff where needed.
- Revision conflicts: return an error with code `Conflict` and an explanation (current revision != expected), with the recommendation to run `get_work_item` and retry.
- Input validation: handled at the MCP tool level, returning 400 with the field and error description.

## 10. Testing and acceptance plan

### Unit tests
- Input message parsing, schema validation, and helper functions (patch generation) are fully covered by unit tests.

### Integration tests
- Test `azure_devops_client` against `wiremock` with canned Azure responses.
- Test retry/timeout/404/5xx behavior.

### End-to-end tests
- E2E against a local Azure DevOps Server (when available), or tests that record and replay real responses (record/replay fixtures).

### Contract tests (stdio)
- Check schema conformance: positive and negative cases; transport timeouts.

### Acceptance criteria for the key FRs
- Create: creates a WI and returns id/url; the fields match.
- Get: returns fields and relations with `$expand`.
- Update: applies the patch; rev increments; a failed test op returns Conflict.
- Search: WIQL returns the expected ids; details → the batch GET returns correct fields.

### Skill content tests
- The skill's command map is compared against the CLI command list in both directions.
- The skills' factual claims (reason tokens, exit codes, JSON paths, on-prem behaviour) are pinned to the implementation.

### CI
- GitHub Actions: fmt, clippy, cargo test (unit + integration), schemars validation, contract tests.

## 11. Risks and limitations

- Azure REST API versioning: different on-prem installations may support different preview versions; keep `api-version` configurable.
- Without attachments and caching in the first release, some user workflows are limited.
- Authentication is mandatory, so a deployment in an untrusted network still depends on the PAT staying out of logs and process arguments.

## 12. Open questions (need confirmation)

1. Do we need bulk operations (bulk create/update)?
2. The default log level and retention requirements for metrics/logs.
3. Whether the skills should also be served over the MCP skills extension (SEP-2640).

## 13. Acceptance checklist (short)

- [ ] Create WI: id+url and fields are returned correctly.
- [ ] Get WI: fields+relations+comments are available with `$expand`.
- [ ] Update WI: the patch applies and rev increments; conflicts are handled correctly.
- [ ] Search WI: WIQL → ids; details → correct records.
- [ ] StdIO tools: schemas validate and match the contract.
- [x] Retry & error handling: covered by tests.
- [x] Agent skills: `ado-cli skills get core` prints the guide byte-exactly, with no token and no network.

## 14. Appendices and links

- Azure DevOps REST API: https://learn.microsoft.com/en-us/rest/api/azure/devops/
- Working with work items (Create/Get/Update/WIQL): https://learn.microsoft.com/en-us/rest/api/azure/devops/wit/
- rmcp crate (refreshed via Context7): https://docs.rs/rmcp/latest/
- Reference implementation (study): https://github.com/like-a-freedom/rusty-intervals-mcp
- Architecture Decision Records: ./adr/

## 15. Recommendation: reuse `azure_devops_rust_api` for the client implementation

> Historical. This recommendation was implemented; `azure_devops_rust_api` is the client in use today.

In short: rather than hand-writing a low-level HTTP client, reuse the official `azure_devops_rust_api` crate. It is auto-generated from the Azure DevOps OpenAPI specification and already contains the `wit` (Work Item Tracking) module, WIQL/search, and the related clients.

Why this is safe and worthwhile:
- It covers many endpoints, including Work Item Tracking and Search.
- It supports authentication through `azure_identity` and PAT (`ADO_TOKEN`), and is tested and maintained by Microsoft.
- It supports feature flags, so only the needed modules (for example `wit`, `work`, `search`) can be enabled to reduce compile size.

How to integrate into `rusty_azure_devops`:
1. Add the dependency to `Cargo.toml` (pin the minimum verified version):

```toml
azure_devops_rust_api = { version = "0.39.1", features = ["wit","core"] }
```

2. Implement the `AzureDevOpsClient` trait (described in the NFR/architecture section) as a thin wrapper over `azure_devops_rust_api`:
- `create_work_item` → call the corresponding `wit` method (build the JSON Patch, POST)
- `get_work_item` → `wit::work_items_client().get(...)` with the needed query params (`$expand`, `fields`)
- `update_work_item` → `wit::work_items_client().update(...)` (JSON Patch)
- `search_work_items` → `wit::wiql_client().query_by_wiql(...)`, then a batch GET when `details=true`

3. Keep retry/backoff, error conversion, and conflict handling (`rev`/`test`) inside the wrapper so the MCP interface stays predictable and easy to test.

4. Testing: write integration tests that drive `azure_devops_rust_api` against mocks (wiremock) or with recorded real requests, and add examples in `examples/`.

Authentication note:
- `azure_devops_rust_api` supports several approaches (az CLI, `azure_identity`, PAT). For on-prem, use a PAT through `ADO_TOKEN` or the `.env` config.

Links:
- Crates.io: https://crates.io/crates/azure_devops_rust_api
- Docs.rs: https://docs.rs/azure_devops_rust_api/latest/azure_devops_rust_api/
- Repository: https://github.com/microsoft/azure-devops-rust-api
