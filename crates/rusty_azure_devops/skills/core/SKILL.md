---
name: core
description: "Drive the ado-cli Azure DevOps CLI — read, search, create, update, link, and comment on work items. Start here for any ado-cli operation."
---

# Azure DevOps CLI

Use `ado-cli` for direct Azure DevOps work-item and project operations.

This skill is served by the binary itself, so it always matches the installed
version of the CLI.

## Before you act

Read the current state before you change it, and read it again after you
change it. A mutation response that carries an ID is not proof the change
landed.

1. **Select the operation.** Read, search, create, update, link, or comment.
   Add `--json` for structured handling.
2. **Read or search** with the narrowest command that establishes the state.
3. **Mutate only when the user asked for it.** Compile the payload from the
   exact `--help` output, not from memory.
4. **Verify** by re-reading the work item and comparing the persisted fields
   and relations with the request.

## Recovery gates

- A tool-input error, transport error, timeout, or truncated response is
  **ambiguous** when a mutation may have been sent. Do not blindly retry a
  create — reconcile by exact title or known ID first.
- On an update, pass `--expected-rev` with the revision you read. A mismatch
  fails the update instead of overwriting a concurrent edit.
- Keep credentials out of commands, prompts, logs, and output. Prefer
  `ADO_TOKEN` or `ado-cli login --token-stdin` over `--token`, which is
  visible in `ps` and shell history.

## Command map

The full command surface — every flag, exit code, and JSON shape — is in
`references/commands.md`. Load it when you need a command shape this file
does not spell out.

| Need | Command |
|---|---|
| Connection and default project | `ado-cli status --json` |
| Projects | `ado-cli list-projects --json` |
| Work item by ID | `ado-cli get-work-item <ID> --json` |
| Description, relations, comments | add `--description`, `--expand relations`, or `--comments` |
| WIQL search | `ado-cli search --project <P> --wiql "<WIQL>" --details --json` |
| Create a work item | `ado-cli create-work-item --project <P> --type <T> --title "<T>"` |
| Update fields | `ado-cli update-work-item <ID> --field '<KEY>=<VALUE>'` |
| Read the discussion | `ado-cli comments <ID> --json` |
| Append to the discussion | `ado-cli add-comment <ID> '<TEXT>' --json` |
| Add or remove a relation | `ado-cli manage-link add\|remove <SOURCE_ID> <TARGET>` |
| Run a saved query | `ado-cli get-query-results --project <P> --query-id <GUID>` |

## References

- `references/commands.md` — the complete command surface.
- `references/wiql.md` — WIQL patterns that work against Azure DevOps.
- `references/fields.md` — work item field reference names and their traps.

## On-prem servers

On Azure DevOps Server (on-prem TFS) several commands behave differently.
Load the `on-prem` skill as well when the server is not `dev.azure.com`:

```bash
ado-cli skills get on-prem
```
