# Extract orchestration from create_work_item into composable helpers

**Status**: proposed

`WorkItemOperations::create_work_item` (work_item.rs:32-100) is a 63-line function mixing validation, patch construction, client call, comment attachment, and re-fetch-after-comments — each step with a different failure mode but tangled in one function body. The re-fetch-after-comments logic (added to fix the `System.CommentCount: 0` bug) is especially hard to follow: a `match` on re-fetch success/failure with a `tracing::warn!` fallback.

We extract three private helpers: `build_patch` (validation + patch ops + relation ops), `attach_comments` (loop + error handling), and `refresh_fields` (re-fetch + fallback). Each is independently testable and reusable — `refresh_fields` is also useful for `update_work_item`.

## Consequences

- Each step's failure mode is visible in its own function — easier to reason about error handling.
- `refresh_fields` is reusable by `update_work_item` (which also needs fresh fields after mutation).
- Three small testable units instead of one 63-line function.
- `create_work_item` becomes a 5-line orchestrator: build → create → attach → refresh → return.
