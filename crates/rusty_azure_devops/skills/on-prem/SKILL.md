---
name: on-prem
description: "Azure DevOps Server (on-prem TFS) quirks for ado-cli: field/expand conflicts, comments API version, History fallback. Load alongside the core skill when the server is on-prem."
---

# Azure DevOps Server (on-prem TFS)

Azure DevOps Server runs older API versions than `dev.azure.com` and rejects
parameter combinations the cloud accepts. Three of those differences change
the command you should run.

- `--expand` and `--fields` cannot be combined. The server rejects the
  combination, so the client drops `fields` when `expand` is set.
  `ado-cli get-work-item <ID> --fields System.Title --expand relations`
  returns the default field set. Pick one.
- `--expand Comments` is not supported. Read the discussion thread with
  `ado-cli comments <ID>`, which uses the dedicated comments API.
- `ado-cli add-comment` falls back to the `System.History` field when the
  comments API is unavailable. The comment lands, but as a history entry
  rather than a discussion entry.

Configuration on-prem:

- The base URL must include the collection path:
  `https://tfs.example.com/tfs/DefaultCollection`.
- Set `ADO_DEFAULT_PROJECT`, or leave the project segment out of the URL. An
  empty project segment causes a 404 or 401.
- A self-signed server needs `ADO_CA_CERT` pointing at the CA certificate.

The full quirk matrix, with what breaks and what to do instead, is in
`references/limitations.md`.

This skill assumes the `core` skill, which carries the workflow and the
command map:

```bash
ado-cli skills get core
```
