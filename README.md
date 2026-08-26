# Azure DevOps MCP Server & CLI

[![CI](https://github.com/<your-org>/rusty_azure_devops/actions/workflows/ci.yml/badge.svg)](https://github.com/<your-org>/rusty_azure_devops/actions)
[![Release](https://img.shields.io/github/v/release/<your-org>/rusty_azure_devops?label=release)](https://github.com/<your-org>/rusty_azure_devops/releases)
![Rust](https://img.shields.io/badge/rust-stable-orange.svg)

Rust-based Azure DevOps client that runs as an **MCP server** (for VS Code Copilot, Claude, etc.) or as a **standalone CLI**. Both interfaces share the same business logic — no duplication.

---

## What's inside

| Crate | Purpose |
|---|---|
| `azure_devops_client` | HTTP client with trait abstraction, retry/backoff, SDK + mock implementations |
| `rusty_azure_devops` | MCP server, CLI binary, tool definitions, shared service layer |

**Two binaries** ship from `rusty_azure_devops`:

| Binary | Role | Transport |
|---|---|---|
| `ado-mcp` | MCP server for AI clients | stdio (RMCP protocol) |
| `ado-cli` | Command-line tool for humans | terminal |

Both call the same operations — `WorkItemOperations` and `ProjectLookup` are the source of truth for business logic.

---

## Tools

| Tool | CLI command | Description |
|---|---|---|
| `get_status` | `ado-cli status` | Server health, config summary |
| `list_projects` | `ado-cli list-projects` | List projects with pagination |
| `create_work_item` | `ado-cli create-work-item` | Create work item with fields, relations, comments |
| `get_work_item` | `ado-cli get-work-item` | Retrieve by ID, select fields, include comments |
| `update_work_item` | `ado-cli update-work-item` | JSON Patch update, optimistic concurrency |
| `manage_work_item_link` | `ado-cli manage-link` | Add/remove relations between work items |
| `search_work_items` | `ado-cli search` | WIQL search, optional full records |
| `get_workitem_comments` | `ado-cli comments` | Fetch discussion thread |
| `get_query_results` | `ado-cli get-query-results` | Execute saved WIQL query, pagination |
| `add_comment` | `ado-cli add-comment` | Add a comment to a work item |

---

## Configuration

### Priority chain

Settings resolve in this order (highest wins):

1. **CLI flags** (`--token`, `--base-url`)
2. **Environment variables**
3. **`config.yaml`** (next to the binary, or `~/.config/rusty_azure_devops/config.yaml`)
4. **Defaults** (none for required fields — server exits with error)

### Config reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `ADO_TOKEN` | **yes** | — | Azure DevOps Personal Access Token |
| `ADO_BASE_URL` | **yes** | — | Base URL (see examples below) |
| `ADO_ORGANIZATION` | no | — | Organization name (cloud only, ignored for on-prem) |
| `ADO_API_VERSION` | no | — | REST API version override |
| `ADO_DEFAULT_PROJECT` | no | — | Default project when not specified in calls |
| `ADO_CA_CERT` | no | — | Path to custom CA certificate (PEM or DER) for on-prem / self-signed servers |

### `ADO_BASE_URL` examples

| Environment | URL |
|---|---|
| Azure DevOps Services (cloud) | `https://dev.azure.com/yourorg` |
| On-prem TFS / Azure DevOps Server | `https://tfs.example.com/tfs/DefaultCollection` |

> On-prem: include the collection path. Set `ADO_DEFAULT_PROJECT` to avoid empty project segments in URLs (causes 404/401).

### Custom CA certificates

For on-prem servers with self-signed or corporate CA certificates, set `ADO_CA_CERT` to the certificate file path (PEM or DER format):

```sh
export ADO_CA_CERT="/path/to/corporate-root-ca.cer"
```

Or add it to `config.yaml`:

```yaml
ado_ca_cert: "/path/to/corporate-root-ca.cer"
```

The certificate is added to the system trust store at startup via `SSL_CERT_FILE`, so both the Azure SDK and direct HTTP clients use it. Supports `.pem`, `.crt`, `.cer`, and `.der` formats.

### YAML config file

The binary looks for `config.yaml` in two locations (first found wins):

1. **Next to the binary** — in the same directory as `ado-mcp` / `ado-cli`
2. **`~/.config/rusty_azure_devops/config.yaml`** — user-level config (XDG-style)

**Cloud (Azure DevOps Services):**

```yaml
ado_token: "your_pat"
ado_base_url: "https://dev.azure.com/yourorg"
ado_organization: "yourorg"          # org name from the URL
ado_default_project: "Demo"
```

**On-prem (TFS / Azure DevOps Server):**

```yaml
ado_token: "your_pat"
ado_base_url: "https://tfs.example.com/tfs/DefaultCollection"
ado_default_project: "MyProject"
# ado_organization is not needed — collection path is already in ado_base_url
```

A ready-to-copy example: [`config.example.yaml`](config.example.yaml).

---

## Quickstart

### Prerequisites

- Rust (stable)
- Cargo
- Optional: Docker

### Install from source

```sh
git clone https://github.com/<your-org>/rusty_azure_devops.git
cd rusty_azure_devops
cargo install --path crates/rusty_azure_devops
```

This builds `ado-mcp` and `ado-cli` binaries into `~/.cargo/bin/`. Verify:

```sh
ado-mcp --help
ado-cli status --include-projects
```

### Run with Cargo (development)

```sh
git clone https://github.com/<your-org>/rusty_azure_devops.git
cd rusty_azure_devops

export ADO_TOKEN="your_pat"
export ADO_BASE_URL="https://dev.azure.com/yourorg"

# MCP server (for AI clients)
cargo run --bin ado-mcp

# CLI
cargo run --bin ado-cli -- status --include-projects
```

### Run with Docker

```sh
docker build -t ghcr.io/<org>/rusty-azure-devops:local .

docker run -i --rm \
  -e ADO_TOKEN="$ADO_TOKEN" \
  -e ADO_BASE_URL="$ADO_BASE_URL" \
  ghcr.io/<org>/rusty-azure-devops:local
```

---

## Deployment

### Binary installation

```sh
# Build and install from local checkout
cargo install --path crates/rusty_azure_devops
```

Binaries land in `~/.cargo/bin/`:

| Binary | Purpose |
|---|---|
| `ado-mcp` | MCP server for AI clients (stdio transport) |
| `ado-cli` | CLI for humans |

### Environment setup

Set required variables in your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) or use a `config.yaml`:

```sh
export ADO_TOKEN="your_pat"
export ADO_BASE_URL="https://dev.azure.com/yourorg"
export ADO_DEFAULT_PROJECT="Demo"          # optional
export ADO_CA_CERT="/path/to/ca.cer"       # optional, on-prem only
```

### Verify installation

```sh
ado-cli status --include-projects
```

### Configure MCP client

Point your MCP client (VS Code, Claude Desktop, Cursor, etc.) to the installed binary. See [MCP server](#mcp-server-vs-code--copilot) for client-specific configs.

---

## CLI usage

### Global flags

```
--token <PAT>        Override ADO_TOKEN
--base-url <URL>     Override ADO_BASE_URL
--json               Output raw JSON instead of formatted text
```

### Commands

```sh
# Status
ado-cli status --show-secrets --include-projects

# List projects
ado-cli list-projects --limit 10

# Get work item (human-readable)
ado-cli get-work-item 123 --comments --description

# Get work item (JSON)
ado-cli get-work-item 123 --json

# Create work item
ado-cli create-work-item --project Demo --type Task --title "Fix CI"

# Update with JSON Patch
ado-cli update-work-item 123 --patch '[{"op":"add","path":"/fields/System.Tags","value":"urgent"}]'

# Manage links
ado-cli manage-link add 100 101 --link-type System.LinkTypes.Related
ado-cli manage-link remove 100 101

# Search
ado-cli search Demo --wiql "SELECT [System.Id] FROM WorkItems WHERE [System.State] = 'Active'" --details

# Comments
ado-cli comments 123

# Get query results
ado-cli get-query-results --url 'https://dev.azure.com/yourorg/MyProject/_queries/query/GUID'

# Add comment
ado-cli add-comment 123 'Build passed'
```

---

## MCP server configuration

Add the following entry to your MCP client's config file.

**Cloud:**

```json
{
  "mcpServers": {
    "ado-mcp": {
      "command": "ado-mcp",
      "env": {
        "ADO_TOKEN": "your_pat",
        "ADO_BASE_URL": "https://dev.azure.com/yourorg",
        "ADO_ORGANIZATION": "yourorg",
        "ADO_DEFAULT_PROJECT": "Demo"
      }
    }
  }
}
```

**On-prem:**

```json
{
  "mcpServers": {
    "ado-mcp": {
      "command": "ado-mcp",
      "env": {
        "ADO_TOKEN": "your_pat",
        "ADO_BASE_URL": "https://tfs.example.com/tfs/DefaultCollection",
        "ADO_DEFAULT_PROJECT": "MyProject"
      }
    }
  }
}
```

> **No `env` section?** Place [`config.yaml`](#yaml-config-file) next to the `ado-mcp` binary instead — it supports the same settings. The binary looks for it in its own directory at startup.

### Client config locations

| Client | Config file |
|---|---|
| VS Code | `~/.vscode/mcp-servers.json` |
| Claude Desktop (macOS) | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Desktop (Windows) | `%APPDATA%\Claude\claude_desktop_config.json` |
| Cursor / Windsurf | Check client docs — typically `~/.cursor/mcp.json` or project-level `.cursor/mcp.json` |

Restart the client. Try:

- `@ado-mcp list projects`
- `@ado-mcp create_work_item project=Demo type=Task fields.{System.Title}="Test"`

---

## Prompt examples

Use natural language with Copilot or Claude — the server maps prompts to tool calls:

- "Find all open bugs in Demo and summarize last comments."
- "Create a Task in Demo titled 'Investigate build failure' with comment 'CI failed on Linux'."
- "Show work item 123 with comments and related items."

---

## Tags & custom fields

Tags are semicolon-separated strings in `System.Tags`:

```json
{"op":"add","path":"/fields/System.Tags","value":"urgent;ops"}
```

Custom fields use their reference name:

```json
{"op":"add","path":"/fields/Custom.EstimationExpectedDate","value":"2026-03-01"}
```

> To append a tag without overwriting: `get_work_item` → read `System.Tags` → `update_work_item` with the combined value.

---

## Architecture

```
 ado-cli (clap) ──┐
                   ├──▶ Modules.work_items ──▶ AzureDevOpsClient trait ──▶ AzureSdkClient (SDK)
 ado-mcp (rmcp) ──┘    Modules.projects     │                                       InMemoryAzureClient (tests)
                         │
                   domain.rs (DTOs + builder)
                   config.rs  (YAML + env)
                   resilience.rs (ResilienceStrategy trait + RetryPolicy)
```

- **`WorkItemOperations`** — 6 async methods for work item / link / comment tools. No IO, no protocol-specific types.
- **`ProjectLookup`** — 2 async methods for project enumeration + status. No IO.
- **`AdoMcpHandler`** — thin adapter: `Parameters<T>` → operations/lookup → `Json<T>`.
- **`cli.rs`** — clap args → operations/lookup → formatted output or `--json`.
- **`Config`** — loads YAML from binary dir, merges env vars, accepts CLI overrides.
- **`ResilienceStrategy`** — object-safe trait with `ProductionResilience` (retry + on-prem fallback) and `NoOpResilience` (tests). `RetryPolicy` provides exponential backoff with jitter.

Refer to [`CONTEXT.md`](CONTEXT.md) for the domain glossary, [`docs/adr/`](docs/adr/) for architectural decision records, and [`docs/SPEC.md`](docs/SPEC.md) for the full specification.

---

## Development

```sh
# Tests
cargo test --workspace

# Lint
cargo fmt
cargo clippy --all-targets --all-features
```

Key test suites:

| Command | What it tests |
|---|---|
| `cargo test -p rusty_azure_devops` | Service logic + MCP adapter + config |
| `cargo test -p azure_devops_client` | HTTP client, retry, mock |


---

## Troubleshooting

| Problem | Fix |
|---|---|
| `missing ADO_TOKEN` / `missing ADO_BASE_URL` | Set env vars, add to `config.yaml`, or pass `--token`/`--base-url` |
| `cargo check` fails on `RoleClient` | Enable `client` feature in `Cargo.toml` (already enabled by default) |


---

## License

Licensed under the [MIT License](LICENSE).

This project is not affiliated with Microsoft or Azure DevOps.
