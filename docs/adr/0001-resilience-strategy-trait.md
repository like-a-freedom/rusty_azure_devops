# ResilienceStrategy trait extracted from AzureSdkClient

**Status**: superseded by [ADR-0007](0007-split-resilience-strategy.md)

`AzureSdkClient` mixed three responsibilities — Azure DevOps protocol logic, retry with backoff, and on-prem HTTP fallback when the SDK can't parse server responses. This made the module shallow: the 8-method interface was nearly as complex as the implementation, and every test had to exercise resilience concerns alongside business logic.

We extracted a `ResilienceStrategy` trait with two methods — `execute` (retry wrapper) and `fallback_get` (direct HTTP bypass of the SDK) — behind which all resilience logic now lives. Two adapters: `ProductionResilience` (retry + on-prem fallback with API-version/auth-variant iteration) and `NoOpResilience` (passthrough for tests). `AzureSdkClient` receives an `Arc<dyn ResilienceStrategy>` through its constructor.

The SDK error heuristic (`is_heuristic_sdk_error` + compensating retry with empty project) stays private in `AzureSdkClient` because it encodes knowledge about Azure SDK quirks, not about transport instability — classifying that as "resilience" would leak Azure-specific protocol knowledge across the seam. Two adapters justify the trait: `AzureSdkClient` needs real resilience in production, tests need a passthrough.
