# Unify CLI and MCP parameter construction into a shared builder

**Status**: proposed

CLI and MCP construct identical `CreateWorkItemParams` through independent code paths — CLI parses `--field`, `--relation`, `--comment` flags and manually assembles `WorkItemFields`, `RelationDto`, and `CommentDto`; MCP receives JSON deserialized directly by serde. Every field conversion bug (e.g., Markdown→HTML for `System.Description`) must be fixed in two places. The CLI's `parse_key_value`, `parse_relation`, `convert_field_description`, and the field→extra-map construction (cli.rs:340-397) duplicate logic that the MCP path gets for free via serde.

We introduce a shared `domain/builder.rs` module with a `CreateWorkItemParamsBuilder` that takes raw inputs (title, description, fields map, relations, comments) and produces `CreateWorkItemParams`. Both CLI and MCP call this builder. The CLI delegates its parsed args to it; MCP continues to deserialize directly (serde is the builder for JSON input).

## Consequences

- Field conversion bugs concentrate in one module (`builder.rs`) instead of two call sites.
- CLI drops ~80 lines of construction logic (field→extra-map, relation assembly, comment DTO wrapping).
- MCP path remains serde-first — no performance overhead from an intermediate builder layer.
- The builder is independently testable with clear input/output contract.
