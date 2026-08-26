# Remove public schema-inspection methods from AdoMcpHandler

**Status**: accepted

`AdoMcpHandler` exposed five public methods — `tool_count`, `tool_names`, `tool_input_schema_json`, `tool_output_schema_json`, `tools_registration_payload` — that re-exposed JSON schemas the `rmcp` macro had already generated from params structs. The only production caller was `main.rs`, where they fed `tracing::debug!` logs. Schema-inspection tests called them as contract checks.

These methods leaked the rmcp macro's internals across the `ServerHandler` seam: callers had to know about both the macro-generated tool router and the handler's public inspection API.

We deleted all five public inspection methods. `main.rs` drops the debug logging (rmcp emits its own diagnostics at `RUST_LOG=debug`). Schema-inspection tests migrate to contract tests that talk to a running MCP server over stdio — the same round-trip a real client uses — so the seam tests cross is the seam clients cross.

## Consequences

`AdoMcpHandler`'s public surface shrinks to the `ServerHandler` trait. Five tests across three files rewrite as stdio round-trip contract tests. A small amount of startup debug feedback is lost unless the operator sets `RUST_LOG=rmcp=debug`.
