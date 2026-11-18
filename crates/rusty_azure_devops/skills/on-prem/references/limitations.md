# On-prem limitations

One row per quirk: what breaks, and what to do instead. Everything here
applies to Azure DevOps Server (on-prem TFS). `dev.azure.com` (cloud) is
not affected.

| Quirk | What happens | What to do instead |
|---|---|---|
| `$expand` and `fields` cannot be combined | The server rejects the combination, so the client drops `fields` when `expand` is set and the server returns every field | Pick one. Use `--expand relations` on its own, or fetch fields without expanding |
| `expand=Comments` is not supported | The request fails | Use `ado-cli comments <ID>`, which reads the thread through the dedicated comments API |
| The comments API needs a different API version | It requires `6.0-preview`, not `6.0` | Nothing to do. The client handles this. You never pass a version yourself |
| The comments API needs the project in the URL | `…/{collection}/{project}/_apis/wit/workItems/{id}/comments`. Without the project segment the server answers 404, even when the thread is empty | Nothing to do. The client uses `ADO_DEFAULT_PROJECT` when it is set, and otherwise reads `System.TeamProject` from the work item itself |
| `add-comment` may fall back to history | When the comments API is unavailable, the comment is written to the `System.History` field instead of the discussion thread | Read the thread with `ado-cli comments <ID>`. On a server with a working comments API this does not happen |
| The REST API version is downgraded | The SDK rewrites `7.1-preview` to the configured version. With no version configured, an on-prem endpoint defaults to `6.0` | Nothing to do. Set `ADO_API_VERSION` only if the server needs a different one |
| `VssVersionOutOfRangeException: the requested REST API version of 7.1 is out of range` | The config pins `ADO_API_VERSION` to a version this server does not support, so SDK calls (`create-work-item`, `update-work-item`, `search`) get a 400 while reads still work | Set `ADO_API_VERSION` to a version the server accepts (this one tops out at `6.0`), or drop the setting and let the on-prem default apply |
| The base URL needs the collection path | `https://host/tfs/DefaultCollection`. Omitting the collection produces a 404 or 401 | Include the collection, and set `ADO_DEFAULT_PROJECT` so the project segment is never empty |
| A self-signed certificate is rejected | TLS validation fails | Set `ADO_CA_CERT` to the CA certificate in PEM or DER form |

## Checking which server you are on

```bash
ado-cli status --show-secrets
```

The reported base URL tells you: `dev.azure.com/<org>` is cloud, anything
else is on-prem.
