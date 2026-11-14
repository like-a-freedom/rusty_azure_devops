# Built-in agent skills: CLI-only, version-matched

**Status**: proposed

The knowledge an AI agent needs to drive `ado-cli` correctly exists only in
places an agent never reads: `AGENTS.md` carries the on-prem TFS limitations
for contributors to this repo, and the hand-written `~/.agents/skills/ado-cli`
describes a CLI whose flags have since moved. That hand-written skill already
drifted: it documents `ado-cli search <PROJECT> --wiql …`, but `search`'s
project is the `--project` flag, not a positional argument. The command still
works when the project is omitted, so the drift is silent.

We add `ado-cli skills`, a CLI-only command that serves agent-facing usage
guides from inside the binary. The guides are `SKILL.md` files with YAML
frontmatter (`name`, `description`) plus optional `references/`, laid out per
the Agent Skills specification, and embedded at compile time with
`include_dir!`. Because the content ships with the binary, the guidance is
rebuilt and re-versioned with the CLI it describes, so the drift above cannot
recur silently.

The surface follows `agent-browser skills get core`: `ado-cli skills list`,
`ado-cli skills get <name> [--full] [--ref <path>]`, and `ado-cli skills path`.
`skills` is dispatched before credential loading, so it works with no token and
no server, which makes it usable as an agent's first command. It is discoverable from
`ado-cli --help`, which lists it and points at it from a footer.

The skills module is a bounded context of its own: it knows nothing about the
Azure DevOps domain (WorkItem, Relation, Patch Document) and depends on
neither `azure_devops_client` nor the operation layer. Domain errors
(`SkillsError`) are raised by the domain; the CLI maps them onto the existing
`CommandError` envelope, so the reason tokens (`skill_not_found`,
`reference_not_found`, and the existing `validation_failed` and `io_error`),
the hints, and the exit codes are the same contract the rest of the CLI
already uses. The `Tool` enum is unchanged: `skills` is a `Command` with no
`Tool`, and `tests/schema_validation.rs` lists it under `CLI_ONLY` so it can
never silently acquire an MCP counterpart.

A test guards the skill against the CLI: the command map in
`core/references/commands.md` is extracted by a line-anchored scan and
compared against the CLI command list in both directions. Removing a command
from the skill fails the build; documenting a command that does not exist fails
it too.

`ADO_CLI_SKILLS_DIR` replaces the bundled catalog, for iterating on skill
text without a rebuild. It is a developer affordance, not a supported
user-facing configuration surface: a missing directory is a hard error rather
than a silent fallback, because a silent fallback would make `skills path`
report a location that is not the one being served.

## Rejected

Serving skills over the MCP SEP-2640 extension (`skills/list`, `skills/get`,
`resources/read`). `rmcp 3.9.0` already supports it natively, so this is a
follow-up rather than a rewrite. A skill's guidance is only worth serving
over MCP if MCP clients actually consume it, and the extension is recent. The
CLI reaches every agent that can run a command, which is the larger audience
today.

`skills install`, writing copies into `~/.claude/skills` and similar. It
duplicates what `get` already does live and reintroduces staleness, the exact
problem this feature exists to remove. The `get` model works with any harness
without harness-specific directories.

Reading skills from a directory next to the binary. Editable without a rebuild,
but it breaks single-file distribution and re-introduces version drift.

A custom clap error handler so that usage errors also honour `--json`. It
would change the contract of every command, not just this one, and belongs in
its own change. Usage errors therefore stay clap's plain text with exit code 2,
while `--json` shapes the runtime errors; the skill documents the difference.

Deriving the skill's command map from `--help` output at build time. Clap's
rendering is presentation (wrapping, indentation, the synthetic `help` entry), so a clap upgrade would break it without any real drift. The CLI command
list the repo already maintains is the declared statement of "these are the
CLI commands".

## Consequences

The skill content becomes a shipped artifact: changing `skills/` changes the
binary, and the release notes should mention it. The `skills` directory stays
small by convention, and its size is visible in the release binary.

`include_dir` embeds every file under `skills/`, including dotfiles, so the
walk skips them, because an operating system must not add content to a skill. Its
`tracked_path` call is a no-op on stable Rust, so a `build.rs` emits
`cargo:rerun-if-changed=skills`; without it, adding a skill file would leave
the binary stale and the skill would silently not ship.

Both binaries link the library and therefore both carry the embedded bytes. A
few kilobytes in `ado-mcp`, which never uses them. The library is the right
home because the integration tests need to reach it.

`AGENTS.md` loses its copy of the on-prem quirk matrix and points at
`ado-cli skills get on-prem` instead. The test guard covers the command map,
not prose, so a developer working on the client reads the matrix from the
skill: one command, one source of truth.
