# Tool surface contract — single `Tool` enum, parity test MCP ⊇ CLI

**Status**: accepted

The set of operations offered by the server was described in three places that had silently drifted: the `#[tool]` attributes in `tools.rs` (9 tools), the `with_instructions` prose in `get_info` (lists 9 tool names as a string), and the `Command` enum in `bin/cli.rs` (8 variants — `GetQueryResults` was added to the MCP handler in commit 4d4a493 but never reached the CLI, violating ADR-0001's agent-first field-parity goal).

We introduced a single `pub enum Tool` in `tools.rs` as the source of truth — `GetStatus`, `ListProjects`, `GetWorkItem`, `CreateWorkItem`, `UpdateWorkItem`, `ManageWorkItemLink`, `SearchWorkItems`, `GetWorkitemComments`, `GetQueryResults`. `get_info::with_instructions` prose is generated from `Tool::all_names()`. A parity test asserts `Tool ⊇ CliCommand` — every CLI `Command` variant names a `Tool` that exists, but not every `Tool` must have a CLI mirror (params differ between paths).

## Consequences

- `Tool` is now in CONTEXT.md. The CLI/MCP parameter asymmetry (`--field KEY=VALUE` vs `CreateWorkItemParams{relations, comments}`) is not a violation — the contract is over operation *names*, not *params*.
- Adding a new tool requires one place (`enum Tool`) plus the corresponding `#[tool]` method; the `get_info` prose updates itself.
- The still-open feature gap "CLI lacks `get_query_results`" is now a feature request, not an architectural drift — the parity test will regenerate its documentation.
