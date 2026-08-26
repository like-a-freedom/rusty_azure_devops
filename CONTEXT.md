# rusty_azure_devops

MCP server and CLI for on-premises Azure DevOps Server — proxies work item, project, and comment operations through the Azure DevOps REST API.

## Language

**WorkItem**:
A tracked unit of work in an Azure DevOps project, identified by an integer id and carrying fields, relations, and comments.
_Avoid_: ticket, issue, task (task is a Work Item Type, not the concept)

**AzureError**:
The error type returned by every `AzureDevOpsClient` method, and by every downstream operation (WorkItemOperations, ProjectLookup, QueryOperations, AdoMcpHandler, CLI) as `Result<_, AzureError>`. Variants: `Http(HttpError)` for transport/SDK failures, `Config(String)`, `NotImplemented`. `HttpError` is a public inner enum with variants `Transport`, `Timeout`, `NotFound`, `Conflict`, `Decode`, `Fallback(Vec<String>)`; its variants are visible to consumers (so tests can `assert!(matches!(err, Err(AzureError::Http(HttpError::Conflict))))`), but the methods that build `HttpError` from `azure_core::Error` are crate-private. Replaces the prior `Result<_, String>` downstream surface.
_Avoid_: error, exception, problem

**Project**:
A team project in Azure DevOps that contains work items, repos, and pipelines.
_Avoid_: collection, team

**Tool**:
One of the ten operations the MCP server exposes to AI agents (`get_status`, `list_projects`, `get_work_item`, `create_work_item`, `update_work_item`, `manage_work_item_link`, `search_work_items`, `get_workitem_comments`, `get_query_results`, `add_comment`). The single source of truth is the `Tool` enum in `tools.rs` — `get_info` prose and the CLI `Command` enum are derived from it; a parity test enforces `Tool ⊇ CliCommand`. The CLI does not have to mirror MCP params 1:1, but every CLI command must name a `Tool` that exists.
_Avoid_: command (the CLI name), tool name, handler method

**Relation**:
A typed link between two Work Items (e.g., `System.LinkTypes.Related`, `System.LinkTypes.Hierarchy-Forward`).
_Avoid_: link (use the verb "to link", but the noun is Relation)

**Comment**:
An entry in a Work Item's discussion thread, with author, text, and creation date.
_Avoid_: note, message

**Patch Document**:
A JSON Patch array of `{op, path, value}` operations sent to Azure DevOps to create or modify a Work Item's fields and relations.
_Avoid_: payload, body

**Write Path**:
The op-naming contract for a Patch Document depends on the operation: `create` emits op `add` (target field/relation does not exist), `update` emits op `replace` (target field exists and is overwritten), and removing a Relation emits op `remove` with a path-indices (`/relations/<idx>`), not a value. The Patch Document is assembled in one place (`domain/conversions.rs`) shared by WorkItemOperations and the CLI; op is chosen per call site, not hard-coded in the builder. `System.Description` values written via the CLI (`--description` flag or `--field 'System.Description=...'`) are auto-converted from Markdown to HTML before patching, because Azure DevOps stores descriptions as HTML.
_Avoid_: "add vs replace inconsistency", "patch operation" (use op)

**Query**:
A saved/shared work item query in Azure DevOps, identified by a GUID (e.g., `1c3ed7b3-17bb-4fe9-9b64-256f118ebebf`) and reachable through a URL like `.../Project/_queries/query/GUID`. Executed via `get_query_results` to return matching work item IDs.
_Avoid_: search, WIQL query (that's `search_work_items`)

## Test doubles

**InMemoryAzureClient**:
The test adapter for `AzureDevOpsClient` — holds work items, projects, comments, and query results in plain collections. Implements the trait directly without depending on `RetryStrategy` or `OnPremFallback`. Configurable per-method error injection through optional error fields. Sensible defaults via `Default`.
_Avoid_: MockAzureClient, MockBuilder, mock client

## Operations

**Modules**:
The pre-built operation instances (WorkItemOperations, LinkOperations, CommentOperations, ProjectLookup, QueryOperations) assembled from a shared `Arc<dyn AzureDevOpsClient>` and `ServiceConfig` in `bootstrap.rs`. Constructed once at startup; handlers and the CLI receive them as a group.
_Avoid_: AppModules, app modules, service registry

**QueryOperations**:
The module that runs saved/shared Azure DevOps queries by URL or (project, query_id) and returns work item IDs. One-method interface (get_query), placed separately from WorkItemOperations to allow future query-related operations (list shared queries, get query by name).
_Avoid_: query service

**WorkItemOperations**:
The module that owns work item CRUD and search — create, update, get, and search. Absorbs Patch Document construction, HTML-to-Markdown conversion (on read via `normalize_description_fields`) and Markdown-to-HTML conversion (on write via `markdown_to_html`). Holds `ServiceConfig` (needs `default_project` for search fallback). Organised as `operations/work_item.rs`; shared micro-helpers (`normalize_description_fields`, `rewrite_missing_fields_error`) live in `operations/helpers.rs`.
_Avoid_: work item service, WI handler

**LinkOperations**:
The module that manages relations between work items — add and remove. Holds `ServiceConfig` (needs `base_url` to construct relation URLs from integer target IDs). Organised as `operations/link.rs`.
_Avoid_: link service, relation handler

**CommentOperations**:
The module that manages work item discussion comments — get and add. Takes only `Arc<dyn AzureDevOpsClient>` — no config needed (comment operations don't require base_url or default_project). Organised as `operations/comment.rs`.
_Avoid_: comment service, discussion handler

**ProjectLookup**:
The module that owns project-level queries — list_projects and get_status. Two-method interface, thin by nature (few project-level operations exist).
_Avoid_: project service

**ServiceConfig**:
Non-secret runtime configuration shared by WorkItemOperations, LinkOperations, and ProjectLookup — base_url, default_project, token_present. Cloned (not Arc-shared) into each module from a single source in bootstrap. CommentOperations does not hold ServiceConfig.
_Avoid_: service context, app config (use for general app-level configuration)

## Resilience

**RetryStrategy**:
How the client adapts to transient transport instability — retries failed SDK calls with exponential backoff and jitter. One-method interface (`retry_policy() -> &RetryPolicy`). Substituted independently of `OnPremFallback`; tests use the default-constructed `NoOpRetry`, production uses `ProductionRetry` (env-driven `RT_RETRY_MAX` / `RT_BASE_DELAY_MS`).
_Avoid_: retry logic, retry handler

**OnPremFallback**:
How the client bypasses the Azure SDK when it cannot parse on-prem Azure DevOps Server responses — a direct HTTP GET that iterates API versions × 2 auth-header variants (`:PAT` and `user:PAT` Basic). One-method interface (`get(path) -> Result<serde_json::Value>`). Substituted independently of `RetryStrategy`; `Option<Arc<dyn OnPremFallback>>` on `AzureSdkClient` — `None` for cloud URLs (which never fall back), `Some(ProductionFallback)` for on-prem. Test adapter `NoOpFallback` returns `NotImplemented`.
_Avoid_: fallback handler, on-prem GET, fallback_get (use the trait method name)

**SDK Error Heuristic**:
A classification of Azure SDK parse errors that indicate an on-prem quirk, triggering a compensating retry with an empty project parameter. Lives in the client, not in `RetryStrategy` or `OnPremFallback`.
_Avoid_: error matcher, quirk detector
