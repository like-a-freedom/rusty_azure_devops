# Contributing

Thanks for your interest in `rusty_azure_devops`.

## Development setup

You need Rust stable and Cargo.

```sh
git clone https://github.com/like-a-freedom/rusty_azure_devops.git
cd rusty_azure_devops

# Run tests
cargo test --workspace

# Format and lint (both must pass before you push)
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

## Pull request process

1. **Open an issue first** for anything non-trivial. Describe the problem, the approach, and the trade-offs.
2. Fork the repo and branch from `main` with a `feat/` or `fix/` prefix.
3. Keep commits focused. Use the imperative mood in the subject line: `Add`, `Fix`, `Update`, `Remove`.
4. Run `cargo test --workspace` and `cargo clippy --all-targets --all-features -- -D warnings` locally.
5. Update the documentation: `README.md`, `CONTEXT.md`, `docs/SPEC.md`, and the relevant ADR.
6. Add an ADR under `docs/adr/` with status `proposed` for any architectural decision. After review and merge, mark it `accepted`.
7. Reference the related issue in the PR description (`Fixes #123`).
8. CI must be green: fmt, clippy, tests, and coverage.

## Code style

- Follow the patterns already in the codebase.
- Run `cargo fmt` before committing.
- Warnings are errors in CI. `cargo clippy --all-targets --all-features -- -D warnings` has to pass.
- Document public functions and modules with `///` doc comments.
- Prefer `Result` and `?` over `unwrap()` and `expect()` in production code. Tests are exempt.
- Avoid `unsafe`. If you need it, document the safety invariants in a `# Safety:` section.
- Keep functions focused and small, and testable in isolation.

## Testing

- Unit tests live beside the code in `#[cfg(test)] mod tests` blocks.
- Integration tests live in `crates/*/tests/` and in `tests/` at the workspace root.
- For Azure SDK calls, use the trait abstraction with `InMemoryAzureClient` as the test double.
- For HTTP-level testing, use `wiremock`, already a dev-dependency of `azure_devops_client`.

## Agent skills are part of the product

`ado-cli skills` serves usage guides that ship inside the binary. A skill is a
shipped artifact, not documentation, so a change to CLI behaviour has to land
with the matching skill edit. `AGENTS.md` carries the table that says which
skill to update for which change.

Two guards keep the skills honest: one checks the skill's command map against
the CLI's command list, and one pins the skills' factual claims to the
implementation.

## Project structure

- `crates/azure_devops_client/` — library crate: trait, SDK adapter, resilience, error types
- `crates/rusty_azure_devops/` — binary crate: MCP server (`ado-mcp`), CLI (`ado-cli`), tool definitions, shared service layer
- `crates/rusty_azure_devops/skills/` — the agent usage guides embedded in the binary
- `docs/adr/` — Architecture Decision Records
- `docs/schemas/` — JSON schemas for the MCP tool contracts
- `docs/SPEC.md` — software requirements specification
- `CONTEXT.md` — domain glossary with the canonical terms
- `AGENTS.md` — agent-facing development guidelines

## Reporting issues

Use GitHub Issues. For security issues, see [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
