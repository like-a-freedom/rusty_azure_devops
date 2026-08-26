# SDK api-version downgrade policy for on-prem servers

**Status**: accepted

`azure_devops_rust_api` 0.38.0 hardcodes `api-version=7.1-preview` in every generated
request builder. On-prem Azure DevOps Server instances support at most API version 6.0,
so all SDK-generated requests fail with a 400 error (`VssVersionOutOfRangeException`).

We introduced `DowngradeApiVersionPolicy` — a custom `azure_core::http::policies::Policy`
that intercepts HTTP requests and rewrites `api-version=7.1-preview` to the configured
version (the first entry in `api_versions`, defaulting to `6.0` when no explicit
`ado_api_version` is set). The policy is attached to both `wit_client` and `core_client`
SDK clients via `.per_try_policies()` in `AzureSdkClient::new()`, only when
`base_contains_collection` is true (on-prem).

**Considered Options**

- **Convert all on-prem methods to reqwest.** Like the existing `fallback_get` path,
  rewrite `create_work_item`, `update_work_item`, `query_wiql`, `batch_get_work_items`,
  `get_comments`, and `add_comment` as raw HTTP calls. Rejected: duplicates SDK
  serialization for every method, adds hundreds of lines of request/response marshalling,
  and creates two independent code paths for every operation.

- **Fork or patch the SDK.** Use a forked `azure_devops_rust_api` with a configurable
  API version. Rejected: maintenance burden of keeping a private fork in sync with
  upstream, and the SDK's autogeneration pipeline doesn't support version overrides.

- **Custom Policy.** Intercept the SDK's HTTP pipeline at the transport layer.
  Single interception point, zero per-method changes, leverages the SDK's own
  extension mechanism. Accepted.

**Consequences**

- `azure_core` is now a direct dependency of `azure_devops_client` at version `1.1.0`
  (chosen because `azure_devops_rust_api` 0.38.0 already depends on it, so the lock
  file resolves to the same version with no dependency bloat).

- On-prem calls through SDK clients (`wit_client`, `core_client`) unconditionally
  rewrite `7.1-preview` to the configured version. The two existing `fallback_get`
  paths (`list_projects`, `get_work_item`) are unaffected because they use reqwest
  directly and already iterate through `api_versions`.

- Added `azure_core::http::policies::Policy` trait to the public dependency surface.
  Reverting this decision means removing the policy, the dependency, and accepting
  that on-prem users must set `ado_api_version: "6.0"` and hope the SDK downstream
  will honor it in a future version.
