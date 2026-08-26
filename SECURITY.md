# Security Policy

## Supported versions

| Version | Supported          |
|---------|--------------------|
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

Please report security issues privately through GitHub Security Advisories:

1. Go to https://github.com/<your-org>/rusty_azure_devops/security/advisories/new
2. Provide a clear description of the vulnerability, including:
   - Affected versions
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

You can expect an acknowledgement within 72 hours and a fix or mitigation plan within 30 days for confirmed vulnerabilities.

## Scope

This project handles Azure DevOps Personal Access Tokens (PATs) for API authentication. Security-sensitive areas include:

- **Credential handling** — PAT storage, transmission, and logging. The server refuses to start without `ADO_TOKEN` set.
- **On-prem TLS** — Custom CA certificate support via `ADO_CA_CERT` / `SSL_CERT_FILE` for self-signed corporate servers.
- **URL handling** — `manage_work_item_link` rejects suspicious target URLs (e.g. web "edit" pages) to prevent unintended navigation patterns.
- **Input validation** — JSON Patch documents and tool parameters are validated against `docs/schemas/*.schema.json`.

## Out of scope

- Vulnerabilities in the underlying `azure_devops_rust_api` SDK — please report to [Azure/azure-sdk-for-rust](https://github.com/Azure/azure-sdk-for-rust).
- Issues in the `rmcp` MCP framework — report upstream.

## Best practices for users

- Use the **minimum-scope** PAT required for your work (Work Items: Read/Write, Project and Team: Read).
- Set `RUST_LOG=warn` in production to avoid leaking request details in logs.
- Prefer `config.yaml` or environment variables over CLI flags (which can appear in shell history).
- Rotate PATs regularly and revoke unused ones.
- Never commit PATs or `config.yaml` (both are in `.gitignore`).
