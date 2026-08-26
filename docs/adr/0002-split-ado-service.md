# AdoService split into WorkItemOperations and ProjectLookup

**Status**: accepted

`AdoService` was a shallow 8-method module that mostly delegated to `AzureDevOpsClient`. The only behaviour it added — Patch Document construction in `create_work_item`, `update_work_item`, and `manage_work_item_link` — leaked across the seam via params DTOs. Two call-sites exist (the MCP handler and the CLI), so collapsing the service into the MCP handler would have forced the CLI to either duplicate patch logic or depend on `rmcp`.

We split `AdoService` into two modules by domain entity, not by CRUD verb. `WorkItemOperations` (6 methods: create, update, link, get, search, comments) absorbs all Patch Document construction, relation formatting, and HTML-to-Markdown conversion — the business logic for the work item domain. `ProjectLookup` (2 methods: `list_projects`, `get_status`) handles project-level queries. Each module holds `Arc<dyn AzureDevOpsClient>` and a cloned `ServiceConfig` (small struct; `Arc` is premature optimisation). `bootstrap.rs` creates one `Arc<dyn AzureDevOpsClient>` and hands it to both. No facade — handler and CLI depend on the modules they need.

## Considered Options

- **Collapse** (move patch builders into `AdoMcpHandler`, delete `AdoService`). Rejected because the CLI also needs patch logic and shouldn't depend on `rmcp`.
- **Deepen** (extract `WorkItemOperations`). Accepted.

## Consequences

MCP handler now holds two fields (`work_items`, `projects`) instead of one. CLI constructs both modules with the same `Arc<dyn AzureDevOpsClient>`. `AdoService` is deleted; tests that referenced `AdoService::new` migrate to `WorkItemOperations::new` / `ProjectLookup::new`.
