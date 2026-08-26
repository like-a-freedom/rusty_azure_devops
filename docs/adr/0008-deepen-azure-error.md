# Deepen `AzureError` — inner `HttpError` enum, propagate to downstream `Result<_, AzureError>`

**Status**: accepted

`AzureError` had three variants — `Http(String)`, `Config(String)`, `NotImplemented`. Every transport failure (timeout, 429, 404, 409, decode, fallback-exhausted) collapsed into `Http(String)`, forcing `is_heuristic_sdk_error` to match substrings of `Display` output and `tests/update_conflict.rs` to `assert!(res.is_err())` instead of distinguishing a 409.

We deepened the enum to `AzureError::Http(HttpError)` + `Config(String)` + `NotImplemented`, where `HttpError` is a public inner enum whose variants are `Transport`, `Timeout`, `NotFound`, `Conflict`, `Decode`, `Fallback(Vec<String>)`. `HttpError` is constructed via an extension trait `AzureErrorExt` on `azure_core::Error` (`fn into_azure(self) -> AzureError`), so ~20 call sites in `client.rs` migrate from `.map_err(|e| AzureError::Http(e.to_string()))` to `.map_err(AzureErrorExt::into_azure)`. `azure_core` does not leak into the public `AzureError` surface — variants wrap categorised detail, not the upstream type.

Downstream surfaces — `WorkItemOperations`, `ProjectLookup`, `QueryOperations`, `AdoMcpHandler`, and the CLI — change from `Result<_, String>` to `Result<_, AzureError>`. Tests replace `is_err()` with `matches!(err, Err(AzureError::Http(HttpError::Conflict)))`. This is a breaking change for the public API; the next release bumps the major version.

## Consequences

- `is_heuristic_sdk_error` is removed; the SDK-parse-error heuristic lives in `AzureErrorExt` as `HttpError::Decode`.
- `InMemoryAzureClient::clone_error` round-tripping through `Display` is deleted — the enum is now correctly `Clone`.
- Callers can now distinguish "retry me" (Transport, Timeout) from "don't retry me" (Conflict, NotFound, Decode) at the type level.
