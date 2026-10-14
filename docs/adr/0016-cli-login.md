# `ado-cli login` as primary auth UX — CLI-only, agent-drivable

Status: proposed

Authentication currently requires hand-editing `config.yaml` with a plaintext PAT
(observed mode 0644) — there is no writer for config. `gcx login`, `pangolin login`
and `keenable login` all solve this with a CLI-only login command, none of which is
exposed as an MCP tool.

We add `ado-cli login` / `ado-cli logout`. Credentials resolve as `--token` >
`--token-stdin` (the `gh --with-token` pattern) > `ADO_TOKEN` env > hidden prompt,
with prompts only on a TTY: a headless run with missing values fails fast with
`missing_value` instead of blocking the caller's turn. Validation runs `list_projects`
before the credentials are written, and a failed validation saves nothing. Existing
config is never overwritten without `--force` (`confirmation_required`, exit 3).

Agent-UX research (dosu ticket protocol + `agent_next_steps`; AX dual-mode paradigm;
`gh --with-token`; voxgig agent-drivable CLI rules) shaped the error contract: `--json`
shapes success *and* failure — errors go to stderr as
`{"ok":false,"reason","detail","hint"}` with stable reason tokens, stdout stays empty
on failure, exit codes are 0/1/2/3, and success carries a `next` verify command. The
`Tool` enum is unchanged (10 variants); `tests/schema_validation.rs` keeps an explicit
`CLI_ONLY` list so login/logout can never acquire an MCP counterpart.

## Rejected

OS-keychain storage (`gh` 2.40+ precedent): the MCP server and CI consume file+env
credentials, so keyring would add platform dependencies and break headless runs; a
0600 file (enforced at creation, re-asserted on rewrite) is the pragmatic baseline.

Browser/device approval flows (dosu/Netlify ticket protocol): impossible against
on-prem TFS PATs — there is no approval authority to bind a session to. The headless
equivalent is env or `--token-stdin`.

## Consequences

`config.rs` gains save helpers and `YamlConfig` becomes `Serialize`; `login.rs` becomes
the flow's single owner (`run_login`/`run_logout` plus testable primitives); one new
dependency (`rpassword`); SPEC gains FR-8 and its §3.1 ENV-only claim is scoped to the
MCP runtime; README gains an agent recipe.
