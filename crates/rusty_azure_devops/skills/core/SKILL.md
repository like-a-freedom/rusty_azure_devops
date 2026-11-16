---
name: core
description: "Drive the ado-cli Azure DevOps CLI: read, search, create, update, link, and comment on work items. Start here for any ado-cli operation."
---

# Azure DevOps CLI

The CLI ships this guide, so it always matches the installed version.

## Working sequence

1. Pick the operation: read, search, create, update, link, or comment. Add
   `--json` when you parse the result.
2. Read or search with the narrowest command that establishes the current
   state.
3. Mutate only when the user asked for it. Build the payload from the exact
   `--help` output.
4. Verify by re-reading the work item and comparing the persisted fields and
   relations against the request. A response carrying an ID is not proof the
   change landed.

## Recovery

- A tool-input error, transport error, timeout, or truncated response leaves
  a mutation ambiguous. Do not retry a create. Reconcile by exact title or
  known ID first.
- On an update, pass `--expected-rev` with the revision you read. A mismatch
  fails the update rather than overwriting a concurrent edit.
- Keep credentials out of commands, prompts, logs, and output. `ADO_TOKEN` and
  `ado-cli login --token-stdin` stay out of argv, which `ps` and shell history
  expose.

## Command map

| Need | Command |
|---|---|
| Connection and default project | `ado-cli status --json` |
| Projects | `ado-cli list-projects --json` |
| Work item by ID | `ado-cli get-work-item <ID> --json` |
| Description, relations, comments | add `--description`, `--expand relations`, or `--comments` |
| WIQL search | `ado-cli search <P> --wiql "<WIQL>" --details --json` |
| Create a work item | `ado-cli create-work-item --project <P> --type <T> --title "<T>"` |
| Update fields | `ado-cli update-work-item <ID> --field '<KEY>=<VALUE>'` |
| Read the discussion | `ado-cli comments <ID> --json` |
| Append to the discussion | `ado-cli add-comment <ID> '<TEXT>' --json` |
| Add or remove a relation | `ado-cli manage-link add\|remove <SOURCE_ID> <TARGET>` |
| Run a saved query | `ado-cli get-query-results --project <P> --query-id <GUID>` |

## References

- `references/commands.md`: every flag, exit code, and JSON shape.
- `references/wiql.md`: WIQL patterns that work against Azure DevOps.
- `references/fields.md`: field reference names and their traps.

## On-prem servers

Azure DevOps Server rejects parameter combinations that `dev.azure.com`
accepts. On any server other than `dev.azure.com`, load the `on-prem` skill
as well:

```bash
ado-cli skills get on-prem
```
