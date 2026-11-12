# WIQL patterns

WIQL is the query language `ado-cli search` speaks. It is not SQL: the
column list is fixed, the field names are bracketed, and there is no
`SELECT *`.

## Shape

```sql
SELECT [System.Id] FROM WorkItems WHERE <condition> ORDER BY <field> ASC
```

`ado-cli search --wiql "..."` runs this. `--details` returns full records
instead of just IDs; `--project` scopes it to one project.

## Field vs. display name

The brackets take a **reference name**, not the display name you see in the
Azure DevOps UI:

| You see | You write |
|---|---|
| ID | `[System.Id]` |
| Title | `[System.Title]` |
| State | `[System.State]` |
| Work Item Type | `[System.WorkItemType]` |
| Assigned To | `[System.AssignedTo]` |
| Area Path | `[System.AreaPath]` |
| Iteration Path | `[System.IterationPath]` |
| Tags | `[System.Tags]` |
| Team Project | `[System.TeamProject]` |
| Created Date | `[System.CreatedDate]` |
| Changed Date | `[System.ChangedDate]` |

`[Id]` also works — Azure DevOps resolves the short form. Prefer the full
reference name; it is unambiguous.

## Conditions

```sql
-- exact match
WHERE [System.State] = 'Active'

-- contains, case-insensitive
WHERE [System.Title] CONTAINS 'login'

-- one of several
WHERE [System.State] IN ('Active', 'New')

-- assigned to a specific identity
WHERE [System.AssignedTo] = 'user@example.com'

-- within a project
WHERE [System.TeamProject] = 'MyProject'

-- combination
WHERE [System.State] = 'Active'
  AND [System.Title] CONTAINS 'timeout'
  AND [System.TeamProject] = 'MyProject'

-- negation
WHERE [System.State] <> 'Closed'

-- date window
WHERE [System.ChangedDate] > '2026-01-01'
```

`CONTAINS` searches title and description. It does **not** search the
comment thread — use `ado-cli comments` for that.

## Ordering and limits

```sql
SELECT [System.Id] FROM WorkItems
WHERE [System.State] = 'Active'
ORDER BY [System.ChangedDate] DESC
```

`--limit` caps the number of results (default 100). Azure DevOps caps
WIQL results server-side too; a `TOP` clause is not supported, so use
`--limit`.

## Saved queries

A saved query is a stored WIQL identified by a GUID. Run it with
`ado-cli get-query-results`, not with `search`:

```bash
ado-cli get-query-results --project MyProject --query-id 1c3ed7b3-17bb-4fe9-9b64-256f118ebebf
ado-cli get-query-results --url 'https://host/MyProject/_queries/query/1c3ed7b3-...'
```

Both `_queries/query/GUID` and `_queries/query-edit/GUID` URL shapes are
accepted.

## Query errors

A WIQL that names a field the project does not have fails with a server
error naming the field. Check the field's reference name against the table
above first — a display name is the usual mistake.
