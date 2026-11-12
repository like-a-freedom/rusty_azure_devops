---
name: on-prem
description: "Azure DevOps Server (on-prem TFS) quirks for ado-cli — field/expand conflicts, comments API version, History fallback. Load alongside the core skill when the server is on-prem."
---

# Azure DevOps Server (on-prem TFS)

On-prem Azure DevOps Server runs older API versions and rejects parameter
combinations that `dev.azure.com` accepts. Several `ado-cli` commands behave
differently there.

**Load the `core` skill as well** — it carries the workflow and the command
map:

```bash
ado-cli skills get core
```

## What changes

- **`--expand` and `--fields` cannot be combined.** The server drops
  `fields` when `expand` is set, so `ado-cli get-work-item <ID> --fields
  System.Title --expand relations` returns the default field set, not the
  one you asked for. Pick one.
- **`--expand Comments` is not supported.** Fetch the discussion thread with
  `ado-cli comments <ID>` instead — it uses the dedicated comments API.
- **The comments API uses a different API version** (`6.0-preview`). This is
  handled inside the client; you do not pass a version yourself.
- **`ado-cli add-comment` falls back** to the `System.History` field when
  the comments API is unavailable. The comment still lands, but it is a
  history entry rather than a discussion entry.
- **The REST API version is downgraded** for on-prem servers (`7.1-preview`
  is rewritten to the configured version, typically `6.0`).

## Configuration

- The base URL must include the collection path:
  `https://tfs.example.com/tfs/DefaultCollection`.
- Set `ADO_DEFAULT_PROJECT`, or leave the project segment out of the URL —
  an empty project segment causes a 404 or 401.
- A self-signed server needs `ADO_CA_CERT` pointing at the CA certificate.

## References

- `references/limitations.md` — the full quirk matrix, one row per quirk with
  its workaround.
