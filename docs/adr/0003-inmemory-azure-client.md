# InMemoryAzureClient replaces MockAzureClient and MockAzureClientBuilder

**Status**: accepted — the reference to `ResilienceStrategy` below is outdated as of [ADR-0007](0007-split-resilience-strategy.md); the trait is now split into `RetryStrategy` + `OnPremFallback`, and the same independence claim holds for both.

Two test doubles existed for `AzureDevOpsClient`: `MockAzureClient` (a zero-field struct with hardcoded returns) and `MockAzureClientBuilder` (13 builder methods configuring one return value each, with a wrapper struct that delegated every trait method). The builder was shallow — 13 public methods for an 8-method interface, and the seam was the builder, not the trait. Tests configured the builder instead of the interface.

We replaced both with a single `InMemoryAzureClient` that implements `AzureDevOpsClient` directly through plain collections: `HashMap<i64, WorkItem>` for create/get/update/batch, static `query_result` for WIQL, `HashMap<i64, Vec<Comment>>` for comments. Patch ops apply to the stored records (minimal state emulation — enough to exercise the service logic without simulating Azure DevOps itself). Per-method error injection via optional `AzureError` fields on the struct. `Default` populates the same sensible defaults the old builder used, so tests override what they care about and rely on `..Default::default()` for the rest.

`InMemoryAzureClient` does not depend on `ResilienceStrategy` — it answers directly from state. Two traits stay independent: `AzureDevOpsClient` (business calls) and `ResilienceStrategy` (transport adaptation).

## Consequences

All tests referencing `MockAzureClient` or `MockAzureClientBuilder` migrate to `InMemoryAzureClient::default()`. The `BadClient` pattern in `tools.rs` tests (a full trait impl targeting one failing method) collapses to `InMemoryAzureClient { update_work_item_error: Some(...), ..Default::default() }`.
