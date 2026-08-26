# Replace manual bootstrap with declarative module registry

**Status**: proposed

`bootstrap.rs` has five identical convenience functions (`work_item_ops`, `project_lookup`, `query_ops`, `link_ops`, `comment_ops`) that each do `X::with_config(client.clone(), config.clone())`. Adding a module requires editing three places: `AppModules` struct, `create_modules`, and a new convenience function. The functions are boilerplate — their only variation is the type name.

We collapse into a `Modules` struct with all operations as public fields, built in `create_modules`. Adding a module = adding a field + one constructor line. The convenience functions are deleted.

## Consequences

- Adding a new operation module requires editing two places (struct + constructor) instead of three.
- `modules.work_items` is self-documenting — no need to remember which convenience function to call.
- Deletes five boilerplate functions (~30 lines).
- `AppModules` → `Modules` rename clarifies intent: these are pre-built domain modules, not a generic "app" bag.
