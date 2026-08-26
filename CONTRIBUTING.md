# Contributing

Thanks for your interest in contributing to `rusty_azure_devops`!

## Development setup

Prerequisites: Rust stable, Cargo.

```sh
git clone https://github.com/<your-org>/rusty_azure_devops.git
cd rusty_azure_devops

# Run tests
cargo test --workspace

# Format and lint (must pass before submitting a PR)
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

## Pull request process

1. **Open an issue first** for non-trivial changes — describe the problem, proposed approach, and any trade-offs.
2. Fork the repo, create a branch from `main` named `feat/short-name` or `fix/short-name`.
3. Keep commits focused. Use imperative mood in the subject line: `Add`, `Fix`, `Update`, `Remove`.
4. Run the full local test suite (`cargo test --workspace`) and `cargo clippy -D warnings` before pushing.
5. Update documentation (README, `CONTEXT.md`, ADR) when behavior or architecture changes.
6. For architectural decisions, add a new ADR under `docs/adr/` with status `proposed`. After review and merge, mark it `accepted`.
7. Reference the related issue in the PR description (e.g. `Fixes #123`).
8. PRs require a green CI run (fmt + clippy + tests + coverage).

## Code style

- Follow existing patterns in the codebase.
- Run `cargo fmt` before committing.
- Run `cargo clippy --all-targets --all-features -- -D warnings` — warnings are errors in CI.
- Document public functions and modules with `///` doc comments.
- Prefer `Result` and `?` over `unwrap()`/`expect()` in production code. Tests are exempt.
- Avoid `unsafe`; if you must use it, document the safety invariants with a `# Safety:` section.
- Keep functions focused and small. Each task should be testable in isolation.

## Testing

- Unit tests live alongside the code in `#[cfg(test)] mod tests` blocks.
- Integration tests live in `crates/*/tests/` and `tests/` at the workspace root.
- For Azure SDK calls, use the trait abstraction with `InMemoryAzureClient` as the test double.
- For HTTP-level testing, use `wiremock` (already in dev-deps of `azure_devops_client`).

## Project structure

- `crates/azure_devops_client/` — library crate: trait, SDK adapter, resilience, error types
- `crates/rusty_azure_devops/` — binary crate: MCP server (`ado-mcp`), CLI (`ado-cli`), tool definitions, shared service layer
- `docs/adr/` — Architecture Decision Records
- `docs/schemas/` — JSON schemas for MCP tool contracts
- `docs/SPEC.md` — software requirements specification
- `CONTEXT.md` — domain glossary (canonical terms)
- `AGENTS.md` — agent-facing development guidelines

## Reporting issues

Use GitHub Issues. For security issues, see [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
