# Split `operations.rs` into `operations/` submodules by domain entity

**Status**: accepted

`operations.rs` grew to 1208 lines — 612 of code, 612 of tests — with three distinct domain responsibilities in one file (`WorkItemOperations`, `ProjectLookup`, `QueryOperations`) plus two inlined helpers (HTML→Markdown field normalisation and the "missing field `fields`" error rewrite) duplicated in `search_work_items` and `get_work_item`.

We reorganised the file into an `operations/` module with submodules by domain entity — `operations/work_item.rs`, `operations/project.rs`, `operations/query.rs` — and extracted the shared micro-helpers into `operations/helpers.rs`. The public surface (the three service structs and their async methods) is unchanged; `operations/mod.rs` re-exports them.

## Consequences

- Module boundaries now mirror the CONTEXT.md domain language (`WorkItemOperations`, `ProjectLookup`, `QueryOperations`), which aids navigation in parallel with the glossary.
- Two duplicated blocks — `normalize_description_to_markdown(&mut WorkItemFields)` and `rewrite_missing_fields_error(&str) -> String` — now live once in `operations/helpers.rs`. Their locality is the `WorkItemOperations` implementation, so they do not leak into `domain/conversions.rs` (which stays the format-conversion seam).
