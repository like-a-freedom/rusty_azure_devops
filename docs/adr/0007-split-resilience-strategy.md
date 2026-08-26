# Split `ResilienceStrategy` into `RetryStrategy` + `OnPremFallback`

**Status**: accepted — supersedes [ADR-0001](0001-resilience-strategy-trait.md)

ADR-0001 bundled retry and on-prem HTTP fallback behind a single `ResilienceStrategy` trait with the argument "two adapters justify the trait." The audit surfaced two flaws: (a) the ADR's documented method was `execute`, but the code has `retry_policy()` — the ADR already diverged from the implementation; (b) `NoOpResilience::fallback_get` returns `AzureError::NotImplemented`, so any test that injects the no-op adapter on an on-prem URL silently loses the fallback path. The single trait forced the two concerns to be substituted together — a leaky seam.

We split the trait into `RetryStrategy` (one method, `retry_policy() -> &RetryPolicy`) and `OnPremFallback` (one method, `get(path) -> Result<serde_json::Value, AzureError>`). `AzureSdkClient` holds `retry: Arc<dyn RetryStrategy>` (required) and `fallback: Option<Arc<dyn OnPremFallback>>` (`None` for cloud URLs that never fall back, `Some(ProductionFallback)` for on-prem). Tests inject the two independently — they can exercise retry without fallback, or fallback without retry.

## Consequences

- ADR-0001 is superseded. The invariant "fallback lives behind a trait" survives, but the trait is now orthogonal to retry.
- As a follow-on effect (not a separate ADR), the duplicated `AzureSdkClient::new` / `with_resilience` constructors collapse into one `AzureSdkClientBuilder` whose `build() -> Result<Self, AzureError>` absorbs all URL-parsing, SDK-client construction, and conditional `DowngradeApiVersionPolicy` attachment. `RetryPolicy::from_env()` is no longer called inside the constructor — `bootstrap.rs` reads the env explicitly and passes `ProductionRetry::from_env()` to the builder. CA-cert configuration moves from `config.apply_ca_cert()`'s `unsafe set_var("SSL_CERT_FILE")` to an explicit builder field, eliminating the process-wide side effect.
- `get_info`'s `with_instructions` string, the `with_resilience` escape hatch, and `NoOpResilience` all migrate to the split adapters.
