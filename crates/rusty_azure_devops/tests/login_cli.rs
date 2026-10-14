use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn unique_home(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ado-login-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn user_config(home: &Path) -> PathBuf {
    home.join(".config/rusty_azure_devops/config.yaml")
}

fn seed_config(home: &Path) {
    let p = user_config(home);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        &p,
        "ado_token: old\nado_base_url: https://old.example.com\n",
    )
    .unwrap();
}

/// Hermetic child: own HOME, no inherited ADO_* env. assert_cmd always gives
/// the child piped (non-TTY) stdin, so the TTY gate is exercised headlessly.
fn login_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("ado-cli").unwrap();
    cmd.env("HOME", home)
        .env_remove("ADO_TOKEN")
        .env_remove("ADO_BASE_URL")
        .env_remove("ADO_ORGANIZATION")
        .env_remove("ADO_API_VERSION")
        .env_remove("ADO_DEFAULT_PROJECT")
        .env_remove("ADO_CA_CERT");
    cmd
}

#[test]
fn login_help_lists_expected_flags() {
    let mut cmd = Command::cargo_bin("ado-cli").unwrap();
    cmd.arg("login").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--base-url"))
        .stdout(predicate::str::contains("--token"))
        .stdout(predicate::str::contains("--token-stdin"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--no-verify"));
}

#[test]
fn login_no_verify_writes_config_and_redacted_json() {
    let home = unique_home("save");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://tfs.example.com/tfs/DefaultCollection",
        "--token",
        "pat-from-flag",
        "--project",
        "Demo",
        "--no-verify",
        "--json",
    ]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert_eq!(
        v["base_url"],
        "https://tfs.example.com/tfs/DefaultCollection"
    );
    assert_eq!(v["validated"].as_bool(), Some(false));
    assert!(v["next"].as_str().unwrap().contains("status"));
    assert!(
        v.get("ado_token").is_none(),
        "token must never appear in output"
    );
    assert!(
        v.get("token").is_none(),
        "token must never appear in output"
    );
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("pat-from-flag"));
    assert!(saved.contains("ado_default_project"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn login_refuses_overwrite_without_force() {
    let home = unique_home("guard");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://x.example.com",
        "--token",
        "new",
        "--no-verify",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    // Guard must not modify the existing file.
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("https://old.example.com"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn guard_json_reports_confirmation_required_with_exit_3() {
    let home = unique_home("guardjson");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://x.example.com",
        "--token",
        "new",
        "--no-verify",
        "--json",
    ]);
    let out = cmd.assert().code(3).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(false));
    assert_eq!(v["reason"], "confirmation_required");
    assert!(v["hint"].as_str().unwrap().contains("--force"));
    // Pipe-clean contract: stdout stays empty on failure.
    assert!(out.stdout.is_empty());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn token_stdin_pipe_saves_without_token_flag() {
    let home = unique_home("stdin");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://s.example.com",
        "--token-stdin",
        "--no-verify",
        "--json",
    ]);
    // MUST call write_stdin: the child blocks on stdin until the pipe closes.
    cmd.write_stdin("piped-pat-123");
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"].as_bool(), Some(true));
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("piped-pat-123"));
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn token_stdin_empty_input_is_rejected() {
    let home = unique_home("stdinempty");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://s.example.com",
        "--token-stdin",
        "--no-verify",
        "--json",
    ]);
    cmd.write_stdin("");
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "missing_value");
    assert!(
        !user_config(&home).exists(),
        "nothing must be saved on failure"
    );
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn headless_missing_token_fails_fast_with_json_envelope() {
    let home = unique_home("headless");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://h.example.com",
        "--no-verify",
        "--json",
    ]);
    // No write_stdin: piped-but-empty stdin, and never a TTY → fail fast, never prompt.
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "missing_value");
    assert!(v["hint"].as_str().unwrap().contains("--token-stdin"));
    assert!(!user_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn login_force_overwrites_and_logout_removes() {
    let home = unique_home("force");
    seed_config(&home);
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://y.example.com",
        "--token",
        "new-pat",
        "--no-verify",
        "--force",
    ]);
    cmd.assert().success();
    let saved = std::fs::read_to_string(user_config(&home)).unwrap();
    assert!(saved.contains("https://y.example.com"));
    let mut out = login_cmd(&home);
    out.arg("logout");
    out.assert()
        .success()
        .stdout(predicate::str::contains("Logged out"));
    assert!(!user_config(&home).exists());
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn logout_without_config_reports_not_logged_in() {
    let home = unique_home("logout");
    let mut cmd = login_cmd(&home);
    cmd.args(["logout", "--json"]);
    let out = cmd.assert().code(1).get_output().clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], "not_logged_in");
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn saved_file_is_owner_only_on_unix() {
    let home = unique_home("perms");
    let mut cmd = login_cmd(&home);
    cmd.args([
        "login",
        "--base-url",
        "https://z.example.com",
        "--token",
        "p",
        "--no-verify",
    ]);
    cmd.assert().success();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(user_config(&home))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn token_and_token_stdin_conflict_is_a_usage_error() {
    let home = unique_home("conflict");
    let mut cmd = login_cmd(&home);
    cmd.args(["login", "--token", "a", "--token-stdin", "--no-verify"]);
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
    std::fs::remove_dir_all(&home).ok();
}
