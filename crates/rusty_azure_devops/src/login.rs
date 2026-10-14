//! First-time authentication for the CLI: resolve PAT + URL, validate against
//! the server, and persist to the user config file. CLI-only (like `gcx login`
//! / `pangolin login` / `keenable login`) — no MCP tool. No clap types here so
//! the flow is testable without a CLI.
//!
//! Agent-UX contract (see ADR-0016): prompts fire only on a TTY and go to
//! stderr; headless runs with missing values fail fast with a classified
//! `CommandError`. Secrets never appear in output — automation should prefer
//! env or `--token-stdin` over `--token` (argv is visible in `ps` and history).

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::config::{Config, YamlConfig, binary_config_path, save_user_config, user_config_path};
use crate::{ListProjectsParams, create_modules};

/// Classified, machine-readable failure for login/logout. `reason` is a stable
/// token agents branch on (`missing_value`, `confirmation_required`,
/// `validation_failed`, `not_logged_in`, `io_error`); `hint` is a
/// copy-pasteable fix. Reported on stderr (JSON envelope with `--json`,
/// `Error:`/`Hint:` prose otherwise); stdout stays empty.
#[derive(Debug, Clone)]
pub struct CommandError {
    pub reason: &'static str,
    pub detail: String,
    pub hint: String,
    pub exit_code: i32,
}

impl CommandError {
    /// Exit-code contract: 3 = confirmation_required (re-run with --force),
    /// 1 = everything else. clap owns 2 (usage errors).
    pub fn new(reason: &'static str, detail: String, hint: String) -> Self {
        let exit_code = match reason {
            "confirmation_required" => 3,
            _ => 1,
        };
        Self {
            reason,
            detail,
            hint,
            exit_code,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "reason": self.reason,
            "detail": self.detail,
            "hint": self.hint,
        })
    }

    /// Print to stderr (payloads stay on stdout) and exit. Never returns.
    pub fn report(self, json: bool) -> ! {
        if json {
            eprintln!("{}", self.to_json());
        } else {
            eprintln!("Error: {}", self.detail);
            eprintln!("Hint: {}", self.hint);
        }
        std::process::exit(self.exit_code);
    }
}

/// Pure merge: explicit flags win over env values. Takes env values as
/// parameters (caller reads env) so this stays side-effect free.
pub fn resolve_credentials(
    flag_url: Option<String>,
    env_url: Option<String>,
    flag_token: Option<String>,
    env_token: Option<String>,
) -> (Option<String>, Option<String>) {
    (flag_url.or(env_url), flag_token.or(env_token))
}

/// Trim an input and reject blanks — single place where "empty means missing"
/// is decided, shared by flag, stdin and prompt sources.
pub fn required(raw: String, label: &str) -> Result<String, String> {
    let v = raw.trim().to_string();
    if v.is_empty() {
        return Err(format!("{label} is required (empty input)"));
    }
    Ok(v)
}

/// Read the whole stdin as a PAT (for `--token-stdin`, the `gh --with-token`
/// pattern). Refuses on a TTY (nothing piped) and on empty input — never falls
/// through to a prompt: when stdin is a pipe there is no terminal to prompt on.
pub(crate) fn read_token_stdin() -> Result<String, String> {
    use std::io::Read;
    if io::stdin().is_terminal() {
        return Err(
            "--token-stdin needs a pipe (e.g. echo \"$PAT\" | ado-cli login --token-stdin ...)"
                .to_string(),
        );
    }
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("cannot read token from stdin: {e}"))?;
    required(buf, "token")
}

pub(crate) fn prompt_line(label: &str) -> Result<String, String> {
    // Prompts go to stderr: stdout stays a clean pipe for payloads.
    eprint!("{label}: ");
    io::stderr()
        .flush()
        .map_err(|e| format!("cannot write prompt: {e}"))?;
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .map_err(|e| format!("cannot read input: {e}"))?;
    required(buf, label)
}

pub(crate) fn prompt_password(label: &str) -> Result<String, String> {
    rpassword::prompt_password(format!("{label}: "))
        .map_err(|e| format!("cannot read token: {e}"))
        .and_then(|v| required(v, "token"))
}

/// Note when a binary-dir config exists: `load_yaml` tries it *first*, so it
/// shadows the user file — and after `logout` a credential may remain there.
pub fn shadow_note(config_path: &Path) -> Option<String> {
    let deployed = binary_config_path()?;
    if deployed.exists() {
        Some(format!(
            "note: {} exists and takes precedence over {} on load — remove it if login seems ignored (or if logout should clear the credential)",
            deployed.display(),
            config_path.display()
        ))
    } else {
        None
    }
}

/// Resolved input for one `login` run. Built from clap flags in `bin/cli.rs`;
/// missing required values are prompted interactively (TTY only).
#[derive(Debug, Clone, Default)]
pub struct LoginParams {
    pub base_url: Option<String>,
    pub token: Option<String>,
    /// Read the PAT from stdin instead of `--token` (keeps the PAT out of
    /// shell history and `ps`). Conflicts with `token`.
    pub token_stdin: bool,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub api_version: Option<String>,
    pub ca_cert: Option<String>,
    pub force: bool,
    pub no_verify: bool,
}

/// Outcome of a successful login. Never contains the token (presence-only,
/// per `status --show_secrets` precedent).
#[derive(Debug, Clone)]
pub struct LoginResult {
    pub config_path: PathBuf,
    pub base_url: String,
    pub validated: bool,
    pub project_count: Option<usize>,
    pub shadow_warning: Option<String>,
}

/// Outcome of a successful logout.
#[derive(Debug, Clone)]
pub struct LogoutResult {
    pub removed: PathBuf,
    pub shadow_warning: Option<String>,
}

const URL_HINT: &str = "re-run with --base-url URL (or set ADO_BASE_URL)";
const TOKEN_HINT: &str = "re-run with --token PAT, --token-stdin (pipe), or set ADO_TOKEN";
const NON_TTY_HINT: &str = "no TTY for interactive prompts — pass flags, --token-stdin, or env";
const STDIN_HINT: &str =
    "pipe the PAT via stdin: echo \"$ADO_TOKEN\" | ado-cli login --base-url URL --token-stdin ...";
const SAVE_HINT: &str = "set HOME and check permissions on ~/.config/rusty_azure_devops";

/// Run the full login flow: guard → resolve → validate → save.
pub async fn run_login(params: LoginParams) -> Result<LoginResult, CommandError> {
    // 1. Guard FIRST — before reading any secret from stdin, so a refused
    //    overwrite never consumes credentials or prompts.
    if let Some(path) = user_config_path()
        && path.exists()
        && !params.force
    {
        return Err(CommandError::new(
            "confirmation_required",
            format!(
                "config already exists at {} — not overwritten",
                path.display()
            ),
            "re-run with --force to overwrite the existing config".to_string(),
        ));
    }

    // 2. Resolve: --token > --token-stdin > ADO_TOKEN > (TTY) prompt.
    let stdin_token = if params.token_stdin {
        Some(
            read_token_stdin()
                .map_err(|d| CommandError::new("missing_value", d, STDIN_HINT.to_string()))?,
        )
    } else {
        None
    };
    let (url, token) = resolve_credentials(
        params.base_url.clone(),
        std::env::var("ADO_BASE_URL").ok(),
        params.token.clone().or(stdin_token),
        std::env::var("ADO_TOKEN").ok(),
    );
    let interactive = io::stdin().is_terminal();
    let url = match url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ if !interactive => {
            return Err(CommandError::new(
                "missing_value",
                "base URL is required".to_string(),
                URL_HINT.to_string(),
            ));
        }
        _ => prompt_line(
            "Azure DevOps base URL (e.g. https://tfs.example.com/tfs/DefaultCollection)",
        )
        .map_err(|d| CommandError::new("io_error", d, NON_TTY_HINT.to_string()))?,
    };
    let pat = match token {
        Some(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ if !interactive => {
            return Err(CommandError::new(
                "missing_value",
                "token is required".to_string(),
                TOKEN_HINT.to_string(),
            ));
        }
        _ => prompt_password("Personal Access Token (PAT, input hidden)")
            .map_err(|d| CommandError::new("io_error", d, NON_TTY_HINT.to_string()))?,
    };

    // Shape-check the URL up front (not a network call, so it applies even
    // with --no-verify): a malformed URL would otherwise panic inside the
    // client builder (exit 101, no envelope).
    url::Url::parse(&url).map_err(|e| {
        CommandError::new(
            "validation_failed",
            format!("invalid base URL {url:?}: {e} (not saved)"),
            "pass a full URL, e.g. https://tfs.example.com/tfs/DefaultCollection".to_string(),
        )
    })?;

    // 3. Validate exactly what gets saved (no env leakage into validation).
    let mut validated = false;
    let mut project_count = None;
    if !params.no_verify {
        let cfg = Config {
            ado_token: pat.clone(),
            ado_base_url: Some(url.clone()),
            ado_organization: params.organization.clone(),
            ado_api_version: params.api_version.clone(),
            ado_default_project: params.project.clone(),
            ado_ca_cert: params.ca_cert.clone(),
        };
        let modules = create_modules(&cfg).map_err(|d| {
            CommandError::new(
                "validation_failed",
                format!("cannot build client for {url}: {d}"),
                "check the base URL, then re-run login".to_string(),
            )
        })?;
        // `total` is the pre-pagination count (operations/project.rs);
        // `projects.len()` would lie when limit < total.
        let list = modules
            .projects
            .list_projects(ListProjectsParams {
                offset: None,
                limit: Some(1),
            })
            .await
            .map_err(|e| {
                CommandError::new(
                    "validation_failed",
                    format!("validation failed for {url}: {e} (not saved)"),
                    "check the base URL and PAT, then re-run login (or add --no-verify to save without checking)".to_string(),
                )
            })?;
        project_count = Some(list.total);
        validated = true;
    }

    // 4. Save, then report a deployed config that would shadow it.
    let yaml = YamlConfig {
        ado_token: Some(pat),
        ado_base_url: Some(url.clone()),
        ado_organization: params.organization.clone(),
        ado_api_version: params.api_version.clone(),
        ado_default_project: params.project.clone(),
        ado_ca_cert: params.ca_cert.clone(),
    };
    let config_path = save_user_config(&yaml)
        .map_err(|d| CommandError::new("io_error", d, SAVE_HINT.to_string()))?;
    let shadow_warning = shadow_note(&config_path);
    Ok(LoginResult {
        config_path,
        base_url: url,
        validated,
        project_count,
        shadow_warning,
    })
}

/// Delete the user config file, reporting any deployed config that still holds
/// a credential (login's mirror image).
pub fn run_logout() -> Result<LogoutResult, CommandError> {
    let path = user_config_path().ok_or_else(|| {
        CommandError::new(
            "io_error",
            "cannot resolve home directory".to_string(),
            "set HOME and re-run logout".to_string(),
        )
    })?;
    if !path.exists() {
        return Err(CommandError::new(
            "not_logged_in",
            "not logged in (no config file found)".to_string(),
            "run `ado-cli login` first".to_string(),
        ));
    }
    std::fs::remove_file(&path).map_err(|d| {
        CommandError::new(
            "io_error",
            format!("cannot remove {}: {d}", path.display()),
            "check permissions on ~/.config/rusty_azure_devops".to_string(),
        )
    })?;
    let shadow_warning = shadow_note(&path);
    Ok(LogoutResult {
        removed: path,
        shadow_warning,
    })
}

#[cfg(test)]
mod tests {
    use crate::login::{CommandError, required, resolve_credentials};

    #[test]
    fn resolve_credentials_prefers_flags_over_env() {
        let (url, token) = resolve_credentials(
            Some("https://flag.example.com".to_string()),
            Some("https://env.example.com".to_string()),
            None,
            Some("env-pat".to_string()),
        );
        assert_eq!(url.as_deref(), Some("https://flag.example.com"));
        assert_eq!(token.as_deref(), Some("env-pat"));
    }

    #[test]
    fn resolve_credentials_all_missing_stays_missing() {
        let (url, token) = resolve_credentials(None, None, None, None);
        assert!(url.is_none());
        assert!(token.is_none());
    }

    #[test]
    fn required_trims_and_rejects_blank() {
        assert_eq!(
            required("  https://x  ".to_string(), "base URL").unwrap(),
            "https://x"
        );
        let err = required("   ".to_string(), "token").unwrap_err();
        assert!(err.contains("token"));
        assert!(err.contains("required"));
    }

    #[test]
    fn command_error_exit_codes() {
        let guard = CommandError::new("confirmation_required", "d".to_string(), "h".to_string());
        assert_eq!(guard.exit_code, 3);
        for reason in [
            "missing_value",
            "validation_failed",
            "not_logged_in",
            "io_error",
        ] {
            let e = CommandError::new(reason, "d".to_string(), "h".to_string());
            assert_eq!(e.exit_code, 1, "reason {reason} must exit 1");
        }
    }

    #[test]
    fn command_error_json_envelope_shape() {
        let e = CommandError::new(
            "missing_value",
            "token is required".to_string(),
            "hint-cmd".to_string(),
        );
        let v = e.to_json();
        assert_eq!(v["ok"].as_bool(), Some(false));
        assert_eq!(v["reason"], "missing_value");
        assert_eq!(v["detail"], "token is required");
        assert_eq!(v["hint"], "hint-cmd");
    }
}
