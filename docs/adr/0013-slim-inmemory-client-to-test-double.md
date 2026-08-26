# Slim InMemoryAzureClient back to a test double

**Status**: proposed

`InMemoryAzureClient` has grown from a simple store-and-return test double into a partial reimplementation of the real client — it now has `Mutex` interior mutability, `apply_patch_to_fields` (50-line JSON Patch applicator), hardcoded `id: 123`, comment persistence with `System.CommentCount` increment, and sequential lock holding across `add_comment`. A test double that reimplements production logic hides bugs in the wrong layer: if the patch applicator in the double has a bug, tests pass but production fails.

We strip `InMemoryAzureClient` back to a simple struct that stores and returns. Comment-count and patch-application correctness are tested through integration tests against the real client (via wiremock), not through a reimplemented double.

## Consequences

- `InMemoryAzureClient` drops from ~390 lines to ~150 lines.
- `apply_patch_to_fields` is deleted — patch logic is tested through the real client's retry/fallback path.
- Comment-count after create is tested via integration tests, not the double.
- Per-method error injection stays — it's the one behavior that belongs in a test double.
- Tests that relied on `InMemoryAzureClient`'s patch logic are migrated to integration tests or simplified.
