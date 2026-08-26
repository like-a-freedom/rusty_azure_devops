# Patch Document module — centralise patch construction in `domain/conversions.rs`

**Status**: accepted

Patch Document construction was hand-rolled inline in three places (`operations.rs::create_work_item`, `operations.rs::update_work_item`, `operations.rs::manage_work_item_link`, and `bin/cli.rs::update-work-item --field`), with divergent op names — CLI emitted `replace`, operations emitted `add`. Azure DevOps treats `add` and `replace` differently on existing fields (`add` fails), so the divergence was a latent correctness gap, not just duplication.

We centralised patch construction behind two functions in `domain/conversions.rs`: `fields_to_patch_ops(fields: &WorkItemFields, op: &str) -> Vec<Value>` and `relation_to_patch(r: &RelationDto, op: &str) -> Value`. Callers pass the op explicitly (`"add"` for create, `"replace"` for update); `manage_work_item_link::remove` still builds its index-based `{op:"remove", path:"/relations/<idx>"}` inline in `operations.rs` because the op requires the relation index, not a value. The fallback patch in `azure_devops_client::client::add_comment` (`{op:"add", path:"/fields/System.History"}`) stays put — it is an HTTP-transport workaround in a crate that cannot depend on `rusty_azure_devops::domain`.

## Consequences

- The op-naming contract (`add` for create, `replace` for update, `remove` with path-index for link removal) is now domain policy, recorded in CONTEXT.md as **Write Path**.
- `operations.rs` and `bin/cli.rs` no longer embed JSON-Patch structure knowledge. CLI `--field` and MCP `UpdateWorkItemParams` emit the same op for the same context.
