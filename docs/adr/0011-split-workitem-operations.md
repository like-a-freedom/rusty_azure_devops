# Split WorkItemOperations into three domain modules

**Status**: accepted

`WorkItemOperations` (996 lines) handles 7 methods across 3 unrelated domain concerns: CRUD+search, link management, and comment operations. The module has no internal seams — every test constructs the full module even when testing a single operation. This makes the module shallow (interface nearly matches implementation) and forces test setup to carry dead weight.

We split into three modules in separate files:

- **`WorkItemOperations`** (`operations/work_item.rs`) — `create_work_item`, `update_work_item`, `get_work_item`, `search_work_items`. Holds `ServiceConfig` (needs `default_project` for search fallback).
- **`LinkOperations`** (`operations/link.rs`) — `manage_work_item_link`. Holds `ServiceConfig` (needs `base_url` to construct relation URLs).
- **`CommentOperations`** (`operations/comment.rs`) — `get_workitem_comments`, `add_comment`. Takes only `Arc<dyn AzureDevOpsClient>` — no config needed.

`rewrite_missing_fields_error` stays in `operations/helpers.rs` as `pub(crate)`, shared by WorkItemOperations and LinkOperations. `normalize_description_fields` stays in helpers.rs, used only by WorkItemOperations.

`AdoMcpHandler` in `tools.rs` holds 5 flat fields (work_items, links, comments, projects, queries) — no facade layer.

## Consequences

- Each module is independently testable with focused fixtures. LinkOperations tests don't need work item seeds; CommentOperations tests don't need config.
- The deletion test passes: removing CommentOperations removes ~80 lines + 6 tests without affecting CRUD or linking.
- `operations/mod.rs` re-exports all three structs. Callers see `WorkItemOperations`, `LinkOperations`, `CommentOperations` — names match CONTEXT.md domain language.
- `bootstrap.rs` gains two convenience constructors: `link_ops()` and `comment_ops()`, alongside existing `work_item_ops()`.
- The `Tool` enum and MCP handler descriptions are unchanged — this is an internal restructure, not a surface change.
