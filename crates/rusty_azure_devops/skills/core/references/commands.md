# Command surface

Every `ado-cli` command, one per line. The line format is fixed: each
command starts a line with `ado-cli `, so the drift guard can read this
file directly.

ado-cli login --base-url https://host/tfs/Coll --token-stdin
ado-cli logout
ado-cli status --json
ado-cli list-projects --json
ado-cli get-work-item 42 --json
ado-cli create-work-item --project P --type Task --title T
ado-cli update-work-item 42 --field 'System.Tags=urgent'
ado-cli manage-link add 42 43 --link-type System.LinkTypes.Related
ado-cli search --project P --wiql "SELECT [Id] FROM WorkItems" --details
ado-cli comments 42 --json
ado-cli add-comment 42 'Build passed'
ado-cli get-query-results --project P --query-id GUID
ado-cli skills get core
ado-cli help skills

## Authentication

`ado-cli login` saves a PAT and the server URL to
`~/.config/rusty_azure_devops/config.yaml` (mode 0600). It is CLI-only, with
no MCP tool behind it.

Resolution order, highest first: `--token`, `--token-stdin`, the `ADO_TOKEN`
environment variable, then a hidden prompt (TTY only).

For automation, prefer the environment variable or `--token-stdin` over
`--token`: an argument is visible in `ps` and in shell history.

`ado-cli login` refuses to overwrite an existing config without `--force`.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime failure |
| 2 | Usage error |
| 3 | Confirmation required: re-run with `--force` |

## JSON output

`--json` shapes the **runtime** errors: they go to stderr as
`{"ok":false,"reason","detail","hint"}` and stdout stays empty. The `reason`
is a stable token: `missing_value`, `confirmation_required`,
`validation_failed`, `not_logged_in`, `io_error`, `skill_not_found`,
`reference_not_found`. The exit code is 1, except `confirmation_required`,
which exits 3 so that a re-run with `--force` is distinguishable from an
ordinary failure.

**Usage errors are not JSON.** An unknown flag, a missing argument, or
`--full` together with `--ref` produces clap's plain-text usage message on
stderr with exit code 2, with or without `--json`.

Success output uses `{"ok":true,...}` for the CLI-only commands (`login`,
`logout`, `skills`). The commands that mirror an MCP tool return the bare
result object instead.

## Command details

### `ado-cli status`

Connection health, project count, and the configured default project.
`--show-secrets` adds the non-sensitive config summary (token presence, base
URL, default project). It never prints the token.

### `ado-cli list-projects`

Lists team projects. `--offset` and `--limit` page through a large list.

### `ado-cli get-work-item`

Returns a curated field set by default: id, rev, type, state, title,
assignee, dates, tags, team project.

- `--fields` selects specific fields, `--fields '*'` returns all.
- `--description` includes `System.Description`, converted from HTML to
  Markdown.
- `--comments` fetches the discussion thread.
- `--expand relations` or `--expand All` uses the Azure API expand.

The Markdown that `--description` returns can be fed straight back into
`--description` or `--field 'System.Description=...'` on create and update;
it is converted back to HTML on the way out.

### `ado-cli create-work-item`

Requires `--project`, `--type`, and `--title`. `--field` sets any field by
reference name and may be repeated. `--relation` adds a relation at creation
time; `--comment` adds a discussion comment.

### `ado-cli update-work-item`

`--field KEY=VALUE` generates a `replace` op and may be repeated.
`--patch` takes a JSON Patch array for anything `--field` cannot express.
`--expected-rev` guards against overwriting a concurrent edit: pass the
revision you read, and the update fails on a mismatch.

`--field` **overwrites**. To set a field to a new value, pass the whole new
value.

### `ado-cli manage-link`

`add` creates a relation, `remove` deletes one. The action, the source ID,
and the target are positional:

```
ado-cli manage-link add <SOURCE_ID> <TARGET> --link-type <TYPE> --comment <REASON>
ado-cli manage-link remove <SOURCE_ID> <TARGET> [--link-type <TYPE>]
```

Link types: `System.LinkTypes.Related` (the default),
`System.LinkTypes.Hierarchy-Forward`, `System.LinkTypes.Dependency-Forward`,
`Microsoft.VSTS.Common.Implementation`, `Hyperlink`.

`remove` is scoped to the given link type, so removing a `Hyperlink` needs
`--link-type Hyperlink`.

### `ado-cli search`

Runs a WIQL query. `--project` defaults to the configured default project.
`--details` returns full records instead of IDs. `--limit` caps the result
count. `--description` includes `System.Description`.

The project is the `--project` **flag**, not a positional argument.

### `ado-cli comments` and `ado-cli add-comment`

`comments` reads the discussion thread of a work item. `add-comment` appends
to it. On on-prem servers `add-comment` falls back to the `System.History`
field when the comments API is unavailable; `comments` has no such fallback
and reports the failure instead.

### `ado-cli get-query-results`

Runs a saved query, by `--url` or by `--project` plus `--query-id GUID`.
`--offset` and `--limit` page through the results. Both `_queries/query/GUID`
and `_queries/query-edit/GUID` URL shapes are accepted.

### `ado-cli skills`

Serves these built-in skills from inside the binary, so they always match
the installed version. `ado-cli skills list`, `ado-cli skills get <NAME>`,
`ado-cli skills get <NAME> --full`, `ado-cli skills path`.
