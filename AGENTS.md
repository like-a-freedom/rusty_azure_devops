
# About This Project

This repository contains an MCP server and CLI for on-premises Azure DevOps Server — proxies work item, project, and comment operations through the Azure DevOps REST API.

## Project Structure

```
crates/
  azure_devops_client/     # Library crate: AzureDevOpsClient trait, SDK adapter, resilience
  rusty_azure_devops/      # Binary crate: MCP server (rmcp), CLI, operations, domain
docs/
  adr/                     # Architecture Decision Records (0001–0015)
  SPEC.md                  # Software Requirements Specification
  schemas/                 # JSON schemas for tool contracts
CONTEXT.md                 # Domain glossary (WorkItem, AzureError, Tool, etc.)
```

### Domain Glossary

See `CONTEXT.md` for canonical terms. Key ones:

- **WorkItem** — tracked unit of work in Azure DevOps (not "ticket", "issue", or "task")
- **AzureError** — error type (`Http(HttpError)` | `Config` | `NotImplemented`); `HttpError` has 6 variants (Transport, Timeout, NotFound, Conflict, Decode, Fallback)
- **Tool** — one of 10 MCP operations (`get_status`, `list_projects`, `get_work_item`, `create_work_item`, `update_work_item`, `manage_work_item_link`, `search_work_items`, `get_workitem_comments`, `get_query_results`, `add_comment`); the `Tool` enum is the single source of truth
- **Relation** — typed link between two Work Items (not "link")
- **Comment** — entry in a Work Item's discussion thread
- **Patch Document** — JSON Patch array `{op, path, value}` sent to Azure DevOps
- **Write Path** — op contract: `add` for create, `replace` for update, `remove` with path-index for link removal
- **Query** — saved/shared work item query identified by GUID

### Architecture

- **`AzureDevOpsClient` trait** — defines all client operations (`get_work_item`, `create_work_item`, `update_work_item`, `get_comments`, etc.); `expand: Option<&str>` on `get_work_item`
- **`AzureSdkClient`** — implementation using `azure_devops_rust_api` SDK; holds `retry: Arc<dyn RetryStrategy>` (required) + `fallback: Option<Arc<dyn OnPremFallback>>` (`None` for cloud, `Some(ProductionFallback)` for on-prem)
- **`AzureSdkClientBuilder`** — single constructor replacing prior `new`/`with_resilience`; URL parsing, SDK construction, `DowngradeApiVersionPolicy` attachment
- **`DowngradeApiVersionPolicy`** — intercepts SDK requests on on-prem to rewrite `7.1-preview` → configured version (e.g. `6.0`)
- **`OnPremFallback` trait** — direct HTTP GET bypassing the SDK for on-prem quirks; `get(path)` + `get_with_api_version(path, version)` methods
- **`InMemoryAzureClient`** — test double implementing `AzureDevOpsClient` with in-memory collections
- **Operations layer** — `WorkItemOperations`, `ProjectLookup`, `QueryOperations` (split per ADR-0010)
- **`AdoMcpHandler`** — MCP handler routing `Tool` variants to operations; parity test enforces `Tool ⊇ CliCommand`

### Known On-Prem TFS Limitations

- `$expand` and `fields` params cannot be used together — client drops `fields` when expand is set
- `expand=Comments` not supported on on-prem — comments fetched separately via comments API
- Comments API requires `api-version=6.0-preview` (not `6.0`), bypassed via `OnPremFallback::get_with_api_version`
- SDK's `DowngradeApiVersionPolicy` rewrites `7.1-preview` → target version; does not handle comments API version

### Key Files

| File | Purpose |
|---|---|
| `crates/azure_devops_client/src/lib.rs` | `AzureDevOpsClient` trait, `AzureError` enum, `WorkItem`/`Comment` structs |
| `crates/azure_devops_client/src/client.rs` | `AzureSdkClient`, `AzureSdkClientBuilder`, all SDK adapter methods |
| `crates/azure_devops_client/src/http_error.rs` | `HttpError` enum (6 variants) |
| `crates/azure_devops_client/src/http_error_ext.rs` | `AzureErrorExt` extension trait |
| `crates/azure_devops_client/src/resilience.rs` | `RetryStrategy`, `OnPremFallback`, `ProductionFallback`, `NoOpFallback` |
| `crates/azure_devops_client/src/api_version_policy.rs` | `DowngradeApiVersionPolicy` |
| `crates/rusty_azure_devops/src/tools.rs` | `Tool` enum, `AdoMcpHandler`, MCP tool definitions |
| `crates/rusty_azure_devops/src/operations/work_item.rs` | `WorkItemOperations` (create, update, link, get, search, comments) |
| `crates/rusty_azure_devops/src/operations/project.rs` | `ProjectLookup` (list_projects, get_status) |
| `crates/rusty_azure_devops/src/operations/query.rs` | `QueryOperations` (get_query) |
| `crates/rusty_azure_devops/src/operations/helpers.rs` | Shared micro-helpers (normalize_description_to_markdown, etc.) |
| `crates/rusty_azure_devops/src/domain/conversions.rs` | Patch Document construction (ADR-0006) |
| `crates/rusty_azure_devops/src/bin/cli.rs` | CLI binary (Command enum, human-readable + JSON output) |
| `crates/rusty_azure_devops/tests/schema_validation.rs` | Parity test: `Tool ⊇ CliCommand` |

### ADRs

Implemented and accepted: 0001 (superseded), 0002–0011. Proposed: 0012–0015. See `docs/adr/`.

| ADR | Title | Status |
|---|---|---|
| 0002 | Split ADO service | accepted |
| 0003 | InMemoryAzureClient | accepted |
| 0004 | Remove handler schema inspection | accepted |
| 0005 | On-prem API version downgrade policy | accepted |
| 0006 | Patch Document module | accepted |
| 0007 | Split ResilienceStrategy → RetryStrategy + OnPremFallback | accepted (supersedes 0001) |
| 0008 | Deepen AzureError | accepted |
| 0009 | Tool surface contract | accepted |
| 0010 | Split operations module | accepted |
| 0011 | Split WorkItemOperations | accepted |
| 0012 | Unify CLI/MCP parameter construction | proposed |
| 0013 | Slim InMemory client to test double | proposed |
| 0014 | Extract orchestration from create_work_item | proposed |
| 0015 | Declarative module registry | proposed |

# Development Guidelines

## Philosophy

### Core Beliefs

- **Incremental progress over big bangs** - Small changes that compile and pass tests
- **Learning from existing code** - Study and plan before implementing
- **Pragmatic over dogmatic** - Adapt to project reality
- **Clear intent over clever code** - Be boring and obvious

### Simplicity

- **Single responsibility** per function/class
- **Avoid premature abstractions**
- **No clever tricks** - choose the boring solution
- If you need to explain it, it's too complex
- Each task should be small, testable, have a clear start and end, and focus on one concern.
- Follow DRY (Don't Repeat Yourself) principle to avoid code duplication, YAGNI (You Aren't Gonna Need It) to avoid over-engineering, and KISS (Keep It Simple, Stupid) to maintain simplicity.

## Technical Standards

### Architecture Principles

- **Composition over inheritance** - Use dependency injection
- **Interfaces over singletons** - Enable testing and flexibility
- **Explicit over implicit** - Clear data flow and dependencies
- **Test-driven when possible** - Never disable tests, fix them
- **Start from monolithic with clear services and modules inside** - Avoid microservices unless justified, but keep monolithic code modular to enable future extraction and make testing easier, with clear boundaries, interfaces, and separation of concerns.

### Error Handling

- **Fail fast** with descriptive messages
- **Include context** for debugging
- **Handle errors** at appropriate level
- **Never** silently swallow exceptions

## Project Integration

### Learn the Codebase

- Find similar features/components
- Identify common patterns and conventions
- Use same libraries/utilities when possible
- Follow existing test patterns
- Strictly follow specification-driven development, have a look at existing specifications in ./docs, and update them as you go.
- After implementation, update specifications to reflect changes.

### Tooling

- Use the project's existing build system, test framework, and formatter/linter settings.
- Ensure CI runs linting, tests, and basic static analysis on every pull request.
- Don't introduce new tools without strong justification and an associated RFC or maintainer approval.

## Code Exploration with Octocode

**Primary method for exploring repositories and understanding code.** Octocode is faster, more economical, and provides structural understanding compared to direct file reading.

### Setup

```bash
# Install octocode
curl -fsSL https://raw.githubusercontent.com/Muvon/octocode/master/install.sh | sh
# or: brew install muvon/tap/octocode

# Set up API key (Voyage AI recommended - 200M free tokens/month)
export VOYAGE_API_KEY="your-voyage-api-key"

# Index the codebase (run once, re-index after major changes)
cd /your/project
octocode index
```

### Key Commands

```bash
# Semantic search - find code by meaning, not keywords
octocode search "authentication middleware"

# View code signatures - see structure without reading entire files
octocode signatures src/middleware/auth.rs

# Get dependency graph - understand relationships between files
octocode graphrag get-relationships --node_id src/middleware/auth.rs

# Find all references to a function/module
octocode graphrag get-references --symbol auth_middleware

# List all symbols in a file
octocode list-symbols src/middleware/auth.rs
```

### Why Octocode Over Direct File Reading

- **Semantic search**: Find relevant code even when you don't know exact file names or function names
- **Knowledge graph**: Understand how modules connect, imports flow, and dependencies work
- **Code signatures**: Quickly assess file structure and exports without loading entire content
- **Persistent memory**: Remember decisions and context across conversations
- **Cross-file navigation**: Trace dependency chains across the entire codebase

### When to Use

- **Exploring unfamiliar codebases** - octocode first, direct file reading only when needed for specific details
- **Finding where features are implemented** - semantic search beats guessing file names
- **Understanding module relationships** - graph traversal reveals architecture
- **Reviewing changes** - see how modifications affect the broader dependency graph

### Code Style

- Follow existing conventions in the project
- Refer to linter configurations and .editorconfig, if present
- Text files should always end with an empty line

## MCP Tool Use

- Use Context7 to validate library documentation and dependency information.
- If web searching is required, use the workspace fetch tools or searxng as a fallback.
- When in doubt about library choices, propose options in an issue and confirm with maintainers.

## Important Reminders

**NEVER**:
- Use `--no-verify` to bypass commit hooks
- Disable tests instead of fixing them
- Commit code that doesn't compile
- Make assumptions - verify with existing code

**ALWAYS**:
- Commit working code incrementally
- Update plan documentation as you go
- Learn from existing implementations
- Stop after 3 failed attempts and reassess

**COMMIT MESSAGE GUIDELINES**
- Use the imperative mood: `Add`, `Fix`, `Update`, `Remove`.
- Reference related issue numbers in the commit or PR description (e.g., `Fixes #123`).

## Quality Gates

### Unit tests (automatic)

Run before every commit. Must pass before code review.

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
```

### E2E test (manual — requires VPN + credentials)

**DO NOT run automatically.** Only when the user explicitly requests it (e.g. "run E2E", "тестируй на сервере").
Requires on-prem TFS access via VPN. Takes ~30-60 seconds.

```bash
cargo build --release -p rusty_azure_devops
CLI=./target/release/ado-cli

# 1. Connection health
$CLI status

# 2. List projects
$CLI list-projects | head -5

# 3. Get work item (default fields)
$CLI get-work-item 9999999

# 4. Get work item with comments (--comments fetches separately on on-prem)
$CLI get-work-item 9999999 --comments

# 5. Get work item with relations (--expand drops fields param on on-prem)
$CLI get-work-item 9999999 --expand relations

# 6. Get comments (dedicated comments API)
$CLI comments 9999999

# 7. Create work item
$CLI create-work-item --project MyProject --type Task --title "E2E test $(date +%s)" --field "System.Description=automated test"

# 8. Update work item (use the ID from step 7)
$CLI update-work-item <NEW_ID> --field "System.State=Closed"

# 9. Search by WIQL
$CLI search --wiql "SELECT [System.Id] FROM WorkItems WHERE [System.Id] = 9999999"

# 10. JSON output
$CLI get-work-item 9999999 --json | head -5

# 11. Manage links (add then remove — cleans up after itself)
$CLI manage-link add 9999999 9999998 --link-type System.LinkTypes.Related
$CLI manage-link remove 9999999 9999998 --link-type System.LinkTypes.Related
```

**Known on-prem TFS limitations:**
- `$expand` and `fields` params cannot be used together — client drops `fields` when expand is set.
- `expand=Comments` is not supported on on-prem — comments are fetched separately via the comments API.
- Comments API requires `api-version=6.0-preview` (not `6.0`), bypassed via `OnPremFallback::get_with_api_version`.
- Write efficient, idiomatic code and avoid needless allocation or complexity. Prioritize correct, maintainable solutions over premature micro-optimizations; document any trade-offs.

You are an expert in Rust, async programming, and concurrent systems. Apply these rules where applicable and keep the language idiomatic to the area you're working in.

Key Principles

- Write clear, concise, and idiomatic Rust code with accurate examples.
- Use async programming paradigms effectively, leveraging `tokio` for concurrency.
- Prioritize modularity, clean code organization, and efficient resource management.
- Use expressive variable names that convey intent (e.g., `is_ready`, `has_data`).
- Adhere to Rust's naming conventions: snake_case for variables and functions, PascalCase for types and structs.
- Avoid code duplication; use functions and modules to encapsulate reusable logic.
- Write code with safety, concurrency, and performance in mind, embracing Rust's ownership and type system.
- Use idiomatic Rust and follow the Rust API Guidelines.
- Prefer Result and Option over unwrap() or panicking.
- Use pattern matching and ? for concise error handling.
- Favor immutability: prefer `let` over `let mut` unless mutation is required.
- Use clippy (cargo clippy --all-targets --all-features -- -D warnings), rustfmt, and cargo check regularly.
- Document public functions and modules with /// doc comments.
- Group related code into crates, modules, and traits.
- - If you need info about dependencies, use #context7 MCP

Async Programming

- Use `tokio` as the async runtime for handling asynchronous tasks and I/O.
- Implement async functions using `async fn` syntax.
- Use `tokio::spawn` judiciously for unit-of-work tasks, preferring scoped tasks and structured concurrency.
- Use `tokio::select!` to coordinate between futures and manage cancellations.
- Implement sensible timeouts, retries, and backoff strategies for robust network and I/O operations.

Channels and Concurrency

- Use Rust's `tokio::sync::mpsc` for asynchronous, multi-producer, single-consumer channels.
- Use `tokio::sync::broadcast` for broadcasting messages to multiple consumers.
- Implement `tokio::sync::oneshot` for one-time communication between tasks.
- Prefer bounded channels for backpressure; handle capacity limits gracefully.
- Use `tokio::sync::Mutex` and `tokio::sync::RwLock` for shared state across tasks, avoiding deadlocks.

Error Handling and Safety

- Embrace Rust's Result and Option types for error handling.
- Use `?` operator to propagate errors in async functions.
- Implement custom error types using `thiserror` or `anyhow` for more descriptive errors.
- Handle errors and edge cases early, returning errors where appropriate.
- Use `.await` responsibly, ensuring safe points for context switching.

Testing

- Write unit tests with `tokio::test` for async tests.
- Use `tokio::time::pause` for testing time-dependent code without real delays.
- Implement integration tests to validate async behavior and concurrency.
- Use mocks and fakes for external dependencies in tests.
- Use cargo test with built-in testing tools.
- Use #[cfg(test)] and #[test] annotations for unit tests.
- Use test modules alongside the code they test (mod tests { ... }).
- Use mockall, fake, or trait-based mocking for services.
- Write integration tests in tests/ with descriptive filenames.
- When using cargo-llvm-cov, ensure to optimize caching like `cargo llvm-cov -p <package_name> --lcov --output-path lcov.info --target-dir target/llvm-cov --skip-clean`


Performance Optimization

- Minimize async overhead; use sync code where async is not needed.
- Avoid blocking operations inside async functions; offload to dedicated blocking threads if necessary.
- Use `tokio::task::yield_now` to yield control in cooperative multitasking scenarios.
- Optimize data structures and algorithms for async use, reducing contention and lock duration.
- Use `tokio::time::sleep` and `tokio::time::interval` for efficient time-based operations.

Key Conventions

1. Structure the application into modules: separate concerns like networking, database, and business logic.
2. Use environment variables for configuration management (e.g., `dotenv` crate).
3. Ensure code is well-documented with inline comments and Rustdoc.

Async Ecosystem

- Use `tokio` for async runtime and task management.
- Leverage `hyper` or `reqwest` for async HTTP requests.
- Use `serde` for serialization/deserialization.
- Use `sqlx` or `tokio-postgres` for async database interactions.
- Utilize `tonic` for gRPC with async support.

Patterns to Follow

- Use modules (mod) and public interfaces (pub) to encapsulate logic.
- Use serde for serialization and thiserror or anyhow for custom errors.
- Implement traits to abstract services or external dependencies.
- Structure async code using async/await and tokio or async-std.
- Prefer enums over flags and states.
- Use builders for complex object creation.
- Split binary and library code (main.rs vs lib.rs) for testability and reuse.

Patterns to Avoid

- Don’t use unwrap() or expect() unless absolutely necessary.
- Avoid panics in library code—return Result instead.
- Don’t rely on global mutable state—use dependency injection or thread-safe containers.
- Avoid deeply nested logic—refactor with functions or combinators.
- Don’t ignore warnings—treat them as errors during CI.
- Avoid unsafe unless required and fully documented.

Security

## 1. Avoid Unsafe Code
- **Rule:** Do not use `unsafe` blocks unless absolutely necessary. If used, document the reason and ensure thorough review.

## 2. Validate All External Input
- **Rule:** All input from users, files, environment variables, or network must be validated for type, length, and format before use.

## 3. Handle Errors Explicitly
- **Rule:** Always handle `Result` and `Option` types explicitly. Do not use `unwrap()` or `expect()` on values that may contain errors or `None`.

## 4. Prevent Integer Overflows
- **Rule:** Use checked arithmetic (`checked_add`, `checked_sub`, etc.) or enable overflow checks in release builds.

## 5. Avoid Panics in Production
- **Rule:** Do not use code that may panic in production environments. Handle errors gracefully and return appropriate error messages.

## 6. Do Not Expose Sensitive Data
- **Rule:** Do not log or expose secrets, credentials, or personal data in error messages or logs.

## 7. Use Strong Types for Security-Critical Data
- **Rule:** Use newtype wrappers or strong types for authentication tokens, passwords, and other sensitive data to avoid accidental misuse.

## 8. Limit Use of Third-Party Crates
- **Rule:** Only use well-maintained and trusted crates. Regularly audit dependencies for vulnerabilities.

## 9. Avoid Dynamic Code Execution
- **Rule:** Do not use crates or patterns that allow dynamic code execution (e.g., `proc_macro`, `eval`-like behavior) with untrusted input.

## 10. Prefer Immutability
- **Rule:** Prefer immutable variables and data structures to reduce the risk of unintended side effects.

Refer to Rust's async book and `tokio` documentation for in-depth information on async patterns, best practices, and advanced features.

## References

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust Style Guide](https://github.com/rust-dev-tools/fmt-rfcs)
- [Tokio Documentation](https://docs.rs/tokio/latest/tokio/)
- [Serde (Serialization Framework)](https://serde.rs/)
- [Clap CLI Framework](https://docs.rs/clap/latest/clap/)
- [Rust Error Handling Patterns](https://docs.rs/anyhow/latest/anyhow/)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)