# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| Latest minor release | :white_check_mark: |
| Earlier releases | :x: |

This project is pre-1.0, so only the newest minor release receives fixes.

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

Report security issues privately through GitHub Security Advisories:

1. Go to https://github.com/like-a-freedom/rusty_azure_devops/security/advisories/new
2. Describe the vulnerability:
   - affected versions
   - steps to reproduce
   - potential impact
   - a suggested fix, if you have one

You can expect an acknowledgement within 72 hours and a fix or mitigation plan
within 30 days for confirmed vulnerabilities.

## Scope

This project handles Azure DevOps Personal Access Tokens (PATs) for API
authentication. The security-sensitive areas:

- **Credential handling** — PAT storage, transmission, and logging. The server refuses to start without `ADO_TOKEN` set.
- **Config file trust** — `config.yaml` (next to the executable or in `~/.config/rusty_azure_devops/`) is refused when group- or world-writable. Newly created config directories get mode `0700`, and existing config files are tightened to `0600` through the open handle before new secret bytes are written.
- **Endpoint validation** — HTTPS is required, with loopback HTTP allowed. URLs with embedded username/password are rejected before the server starts.
- **On-prem TLS** — per-client custom CA certificate support through `ADO_CA_CERT`. Process-wide TLS environment settings are left alone.
- **Terminal output escaping** — human-readable CLI output escapes control characters (ESC, BEL, CR) in server-sourced text, so a fetched Work Item or comment cannot inject terminal escape sequences. JSON output is structured.
- **URL handling** — `manage_work_item_link` rejects suspicious target URLs such as web "edit" pages, which would otherwise cause unintended navigation.
- **Input validation** — JSON Patch documents and tool parameters are validated against `docs/schemas/*.schema.json`.
- **Bundled agent skills** — the skills in `ado-cli skills` are compiled into the binary. `ADO_CLI_SKILLS_DIR` replaces them with a directory you choose, so that directory is trusted input: it is read as UTF-8 text and printed to your terminal. Point it at content you control.

## Out of scope

- Vulnerabilities in the underlying `azure_devops_rust_api` SDK. Report those to [Azure/azure-sdk-for-rust](https://github.com/Azure/azure-sdk-for-rust).
- Issues in the `rmcp` MCP framework. Report those upstream.

## Best practices for users

- Use the **minimum-scope** PAT your work needs (Work Items: Read/Write, Project and Team: Read).
- Set `RUST_LOG=warn` in production so request details stay out of logs.
- Prefer `config.yaml` or environment variables over CLI flags, which can appear in shell history.
- Rotate PATs regularly and revoke the ones you no longer use.
- Never commit PATs or `config.yaml`. Both are in `.gitignore`.
