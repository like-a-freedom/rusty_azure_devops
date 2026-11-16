# Azure DevOps MCP Server & CLI

[![CI](https://github.com/like-a-freedom/rusty_azure_devops/actions/workflows/ci.yml/badge.svg)](https://github.com/like-a-freedom/rusty_azure_devops/actions)
[![Release](https://img.shields.io/github/v/release/like-a-freedom/rusty_azure_devops?label=release)](https://github.com/like-a-freedom/rusty_azure_devops/releases)
![Rust](https://img.shields.io/badge/rust-stable-orange.svg)

Rust-based Azure DevOps client that runs as an **MCP server** (for VS Code
Copilot, Claude, and other MCP clients) or as a **standalone CLI**. Both
interfaces call the same operations, so the behaviour they expose cannot
diverge.

---

## What's inside

| Crate | Purpose |
|---|---|
| `azure_devops_client` | HTTP client with trait abstraction, retry/backoff, SDK + mock implementations |
| `rusty_azure_devops` | MCP server, CLI binary, tool definitions, shared service layer |

Two binaries ship from `rusty_azure_devops`:

| Binary | Role | Transport |
|---|---|---|
| `ado-mcp` | MCP server for AI clients | stdio (RMCP protocol) |
| `ado-cli` | Command-line tool for humans and agents | terminal |

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

## Agent skills

The CLI ships usage guides for AI agents, embedded in the binary so they
always match the installed version:

```sh
ado-cli skills list            # available skills
ado-cli skills get core        # print a skill
ado-cli skills get core --full # with its reference files
ado-cli skills path            # where the catalog came from
```

No token, no network, no install step. The `core` skill covers the workflow
and the command surface; the `on-prem` skill covers Azure DevOps Server (TFS)
quirks. Run `ado-cli skills --help` for the full surface.

---

## Configuration

### First run

```sh
ado-cli login
# prompts for base URL + PAT (hidden input), validates, saves to
# ~/.config/rusty_azure_devops/config.yaml (mode 0600).
ado-cli status --include-projects
```

### For AI agents (non-interactive)

```sh
echo "$ADO_TOKEN" | ado-cli login --base-url URL --token-stdin --force --json
```

Prefer `ADO_TOKEN` or `--token-stdin` over `--token`: an argument is visible
in `ps` and in shell history.

Exit codes are `0` for success, `2` for a usage error, `3` when `--force` is
needed, and `1` for everything else. Runtime errors print JSON on stderr as
`{"ok":false,"reason","detail","hint"}` with a stable `reason`
(`missing_value`, `confirmation_required`, `validation_failed`,
`not_logged_in`, `io_error`, `skill_not_found`, `reference_not_found`) and a
copy-pasteable `hint`. Stdout stays empty on failure.

### Priority chain

Settings resolve in this order, highest wins:

1. **CLI flags** (`--token`, `--base-url`)
2. **Environment variables**
3. **`config.yaml`** (written by `ado-cli login`; next to the binary, or `~/.config/rusty_azure_devops/config.yaml`)
4. **Defaults** — none for required fields, the server exits with an error instead

### Config reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `ADO_TOKEN` | **yes** | — | Azure DevOps Personal Access Token |
| `ADO_BASE_URL` | **yes** | — | Base URL (see examples below) |
| `ADO_ORGANIZATION` | no | — | Organization name (cloud only, ignored for on-prem) |
| `ADO_API_VERSION` | no | — | REST API version override |
| `ADO_DEFAULT_PROJECT` | no | — | Default project when not specified in calls |
| `ADO_CA_CERT` | no | — | Path to custom CA certificate (PEM or DER) for on-prem / self-signed servers |
| `ADO_CLI_SKILLS_DIR` | no | — | Replaces the bundled agent skills with a directory of your own |

### `ADO_BASE_URL` examples

| Environment | URL |
|---|---|
| Azure DevOps Services (cloud) | `https://dev.azure.com/yourorg` |
| On-prem TFS / Azure DevOps Server | `https://tfs.example.com/tfs/DefaultCollection` |

On-prem, include the collection path. Set `ADO_DEFAULT_PROJECT` so a project
segment is never empty, which causes a 404 or 401.

### Custom CA certificates

On-prem servers with self-signed or corporate CA certificates need
`ADO_CA_CERT` pointing at the certificate file (PEM or DER):

```sh
export ADO_CA_CERT="/path/to/corporate-root-ca.cer"
```

Or add it to `config.yaml`:

```yaml
ado_ca_cert: "/path/to/corporate-root-ca.cer"
```

The certificate is loaded into the Azure SDK and the on-prem fallback HTTP
clients for this process. The process-wide `SSL_CERT_FILE` environment
variable is left alone.

### YAML config file

The binary looks for `config.yaml` in two locations, first found wins:

1. **Next to the binary**, in the same directory as `ado-mcp` / `ado-cli`
2. **`~/.config/rusty_azure_devops/config.yaml`**, written by `ado-cli login`. Do not hand-edit secrets here.

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
# ado_organization is not needed — the collection path is already in ado_base_url
```

A ready-to-copy example: [`config.example.yaml`](config.example.yaml).

---

## Quickstart

### Prerequisites

- Rust (stable) and Cargo
- Optionally, Docker

### Install from source

```sh
git clone https://github.com/like-a-freedom/rusty_azure_devops.git
cd rusty_azure_devops
cargo install --path crates/rusty_azure_devops
```

This builds `ado-mcp` and `ado-cli` into `~/.cargo/bin/`. Verify:

```sh
ado-mcp --help
ado-cli status --include-projects
```

### Run with Cargo (development)

```sh
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

The image ships `ado-mcp` only.

---

## CLI usage

### Global flags

```
--token <PAT>        Override ADO_TOKEN
--base-url <URL>     Override ADO_BASE_URL
--json               Output JSON instead of formatted text
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

# Search (the project is the first positional argument)
ado-cli search Demo --wiql "SELECT [System.Id] FROM WorkItems WHERE [System.State] = 'Active'" --details

# Comments
ado-cli comments 123

# Get query results
ado-cli get-query-results --url 'https://dev.azure.com/yourorg/MyProject/_queries/query/GUID'

# Add comment
ado-cli add-comment 123 'Build passed'

# Agent skills
ado-cli skills list
ado-cli skills get core --full
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

Without an `env` section, place a [`config.yaml`](#yaml-config-file) next to the
`ado-mcp` binary instead. It accepts the same settings, and the binary looks
for it in its own directory at startup.

### Client config locations

| Client | Config file |
|---|---|
| VS Code | `~/.vscode/mcp-servers.json` |
| Claude Desktop (macOS) | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Claude Desktop (Windows) | `%APPDATA%\Claude\claude_desktop_config.json` |
| Cursor / Windsurf | Check the client docs, usually `~/.cursor/mcp.json` or a project-level `.cursor/mcp.json` |

Restart the client, then try:

- `@ado-mcp list projects`
- `@ado-mcp create_work_item project=Demo type=Task fields.{System.Title}="Test"`

---

## Prompt examples

Use natural language with Copilot or Claude; the server maps prompts to tool
calls:

- "Find all open bugs in Demo and summarize the last comments."
- "Create a Task in Demo titled 'Investigate build failure' with comment 'CI failed on Linux'."
- "Show work item 123 with comments and related items."

---

## Tags and custom fields

Tags are semicolon-separated strings in `System.Tags`:

```json
{"op":"add","path":"/fields/System.Tags","value":"urgent;ops"}
```

Custom fields use their reference name:

```json
{"op":"add","path":"/fields/Custom.EstimationExpectedDate","value":"2026-03-01"}
```

To append a tag without overwriting, read `System.Tags` with `get_work_item`,
then send the combined value in `update_work_item`.

---

## Architecture

```
 ado-cli (clap) ──┐
                   ├──▶ operations/* ──▶ AzureDevOpsClient trait ──▶ AzureSdkClient (SDK)
 ado-mcp (rmcp) ──┘    work_item, link,      │                       InMemoryAzureClient (tests)
                        comment, project,     │
                        query                 │
                   domain/ (DTOs, params, builder, conversions)
                   config.rs  (YAML + env)
                   skills/    (embedded agent guides)
                   resilience (RetryStrategy + OnPremFallback)
```

- **`WorkItemOperations`** — work item CRUD and search, plus relations and comments. Owns Patch Document construction and the HTML/Markdown conversion.
- **`LinkOperations`** — adds and removes relations between work items.
- **`CommentOperations`** — reads and appends discussion comments.
- **`ProjectLookup`** — project enumeration and server status.
- **`QueryOperations`** — runs a saved query and returns matching IDs.
- **`AdoMcpHandler`** — a thin adapter from `Parameters<T>` to the operations, returning `Json<T>`.
- **`skills/`** — the agent usage guides embedded in the binary, served by `ado-cli skills`.
- **`Config`** — loads YAML from the binary directory, merges environment variables, and accepts CLI overrides.
- **`RetryStrategy`** and **`OnPremFallback`** — two seams for resilience: exponential backoff with jitter, and the direct HTTP GET that bypasses the SDK for on-prem quirks.

[`CONTEXT.md`](CONTEXT.md) holds the domain glossary,
[`docs/adr/`](docs/adr/) the architectural decision records, and
[`docs/SPEC.md`](docs/SPEC.md) the full specification.

---

## Development

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Test suites:

| Command | What it covers |
|---|---|
| `cargo test -p azure_devops_client` | HTTP client, retry, on-prem fallback, mock |
| `cargo test -p rusty_azure_devops --lib` | Operations, domain, skills module |
| `cargo test -p rusty_azure_devops --test schema_validation` | MCP tool schemas, the CLI-to-tool parity check, the skill command-map guard |
| `cargo test -p rusty_azure_devops --test skills_cli` | The `ado-cli skills` command surface |
| `cargo test -p rusty_azure_devops --test skill_content` | The skills' factual claims against the implementation |

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `missing ADO_TOKEN` / `missing ADO_BASE_URL` | Run `ado-cli login`, set the environment variables, add them to `config.yaml`, or pass `--token` and `--base-url` |
| `cargo check` fails on `RoleClient` | Enable the `client` feature in `Cargo.toml`. It is on by default |
| A skill mentions a flag the CLI no longer has | Run `ado-cli skills get core` to read the version that shipped with your binary |

---

## License

Licensed under the [MIT License](LICENSE).

This project is not affiliated with Microsoft or Azure DevOps.
