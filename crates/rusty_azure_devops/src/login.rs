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
use std::path::Path;

use crate::config::binary_config_path;

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
#[allow(dead_code)] // Wired by Task 3 orchestration; remove the allow there.
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

#[allow(dead_code)] // Wired by Task 3 orchestration; remove the allow there.
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

#[allow(dead_code)] // Wired by Task 3 orchestration; remove the allow there.
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

#[cfg(test)]
mod tests {
    use crate::login::{CommandError, resolve_credentials, required};

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
        let guard =
            CommandError::new("confirmation_required", "d".to_string(), "h".to_string());
        assert_eq!(guard.exit_code, 3);
        for reason in ["missing_value", "validation_failed", "not_logged_in", "io_error"] {
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
