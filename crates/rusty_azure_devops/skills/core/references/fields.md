# Work item fields

Fields are addressed by **reference name**: `System.Title`, not "Title".
The reference name is what `--field` and WIQL take.

## Common fields

| Reference name | Notes |
|---|---|
| `System.Id` | The work item's number. Read-only. |
| `System.Title` | Short summary. Required on create. |
| `System.Description` | Long description. See the round-trip note below. |
| `System.State` | `New`, `Active`, `Resolved`, `Closed`, `Removed`. |
| `System.WorkItemType` | `Task`, `Bug`, `Requirement`, `User Story`, `Epic`, `Feature`, `Issue`. Read-only. |
| `System.AssignedTo` | An identity, such as `user@example.com`. |
| `System.AreaPath` | `Project\Team`. Backslashes must be escaped in a shell. |
| `System.IterationPath` | Sprint path. |
| `System.Tags` | Semicolon-separated: `urgent;ops`. |
| `System.TeamProject` | The owning project. Read-only. |
| `System.CreatedDate` | Read-only. |
| `System.ChangedDate` | Read-only. |
| `System.CreatedBy` | Read-only. |
| `System.ChangedBy` | Read-only. |
| `System.Reason` | Why the state changed. |
| `System.BoardColumn` / `System.BoardColumnDone` | Kanban column. |
| `Microsoft.VSTS.Common.Priority` | Priority. |
| `Microsoft.VSTS.Scheduling.StoryPoints` | Story points. |
| `Microsoft.VSTS.CMMI.RequirementType` | `Functional`, `Business Requirement`, … |
| `Custom.*` | Process-specific fields. |

## Setting fields

```bash
ado-cli create-work-item --project MyProject --type Task --title 'Fix login' \
  --field 'System.Tags=urgent' \
  --field 'Microsoft.VSTS.Common.Priority=1'
```

`--field` may be repeated. Every `--field` value is a string; the server
coerces it to the field's type.

`--field` **overwrites**. To change a field, pass the complete new value,
not a delta.

## Read-only fields

`System.Id`, `System.WorkItemType`, `System.TeamProject`, `System.CreatedDate`,
`System.ChangedDate`, `System.CreatedBy`, and `System.ChangedBy` are
server-managed. Setting them fails the patch.

## The `System.Description` round-trip

Azure DevOps stores descriptions as **HTML**. The CLI converts in both
directions so that the text you write and the text you read are the same
Markdown:

- Reading: `ado-cli get-work-item <ID> --description` returns
  `System.Description` converted from HTML to Markdown.
- Writing: `ado-cli create-work-item --description "..."` and
  `ado-cli update-work-item <ID> --field 'System.Description=...'` convert
  the Markdown back to HTML before patching.

So the Markdown that comes out of a read can be fed straight back into a
write. In `--json` the description sits under `fields`, keyed by its
reference name:

```bash
DESC=$(ado-cli get-work-item 42 --description --json | jq -r '.fields["System.Description"]')
ado-cli update-work-item 42 --field "System.Description=$DESC"
```

The conversion is lossy in one direction: HTML that Markdown cannot express
(tables, embedded images, complex styling) is flattened when read. Write the
description as Markdown, not as pasted HTML.

## Area and iteration paths

Backslashes separate path segments, so they must be escaped for the shell:

```bash
ado-cli create-work-item --project MyProject --type Task --title 'Fix login' \
  --field 'System.AreaPath=MyProject\Team'
```

In single quotes the backslash is literal, which is what the server wants.

## Custom fields

`Custom.*` fields come from the inherited process of the target project.
Their reference names are visible in the Azure DevOps UI under the field's
"Reference name" attribute. A `Custom.*` field that the project's process
does not define is rejected by the server.
