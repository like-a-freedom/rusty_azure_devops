use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

/// A child process with a clean environment: no inherited `ADO_*` variables,
/// so the embedded catalog is what gets tested.
fn ado_cli() -> Command {
    let mut cmd = Command::cargo_bin("ado-cli").unwrap();
    cmd.env_remove("ADO_TOKEN")
        .env_remove("ADO_BASE_URL")
        .env_remove("ADO_ORGANIZATION")
        .env_remove("ADO_API_VERSION")
        .env_remove("ADO_DEFAULT_PROJECT")
        .env_remove("ADO_CA_CERT")
        .env_remove("ADO_CLI_SKILLS_DIR");
    cmd
}

#[test]
fn skills_list_works_without_credentials() {
    ado_cli()
        .arg("skills")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("core"))
        .stdout(predicate::str::contains("on-prem"));
}

#[test]
fn skills_list_json_lists_both_skills() {
    let out = ado_cli()
        .args(["skills", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], serde_json::json!(true));
    let names: Vec<&str> = v["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"core"), "{names:?}");
    assert!(names.contains(&"on-prem"), "{names:?}");
}

#[test]
fn skills_get_prints_the_skill_markdown() {
    let out = ado_cli()
        .args(["skills", "get", "core"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("---\nname: core\n"), "{text}");
    assert!(text.contains("# Azure DevOps CLI"), "{text}");
}

#[test]
fn skills_get_json_carries_name_description_and_content() {
    let out = ado_cli()
        .args(["skills", "get", "core", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], serde_json::json!(true));
    assert_eq!(v["name"], serde_json::json!("core"));
    assert!(!v["description"].as_str().unwrap().is_empty());
    assert!(v["content"].as_str().unwrap().contains("name: core"));
}

#[test]
fn skills_get_ref_prints_one_file() {
    let out = ado_cli()
        .args(["skills", "get", "core", "--ref", "references/commands.md"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("ado-cli get-work-item"), "{text}");
    assert!(!text.contains("# Azure DevOps CLI"), "{text}");
}

#[test]
fn skills_get_full_marks_each_file_with_its_path() {
    let out = ado_cli()
        .args(["skills", "get", "core", "--full"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("--- references/commands.md ---"), "{text}");
}

#[test]
fn skills_bare_runs_the_list() {
    let out = ado_cli()
        .arg("skills")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("core"), "{text}");
}

#[test]
fn root_help_mentions_skills() {
    ado_cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("skills"))
        .stdout(predicate::str::contains("Agent skills:"));
}

#[test]
fn skills_help_lists_subcommands() {
    ado_cli()
        .args(["skills", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("get"))
        .stdout(predicate::str::contains("path"));
}

#[test]
fn skills_get_help_documents_flags() {
    ado_cli()
        .args(["skills", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--full"))
        .stdout(predicate::str::contains("--ref"));
}

#[test]
fn unknown_skill_is_reported_as_json_on_stderr() {
    let out = ado_cli()
        .args(["skills", "get", "nope", "--json"])
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(out.stdout.is_empty(), "stdout must stay empty on failure");
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["ok"], serde_json::json!(false));
    assert_eq!(v["reason"], serde_json::json!("skill_not_found"));
    assert!(v["hint"].as_str().unwrap().contains("skills list"));
}

#[test]
fn unknown_skill_without_json_is_prose_on_stderr() {
    ado_cli()
        .args(["skills", "get", "nope"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Hint:"));
}

#[test]
fn unknown_reference_is_reported_as_json_on_stderr() {
    let out = ado_cli()
        .args(["skills", "get", "core", "--ref", "nope.md", "--json"])
        .assert()
        .failure()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], serde_json::json!("reference_not_found"));
}

#[test]
fn full_and_ref_together_is_a_usage_error() {
    ado_cli()
        .args(["skills", "get", "core", "--full", "--ref", "x.md"])
        .assert()
        .code(2);
}

/// A temp directory, created fresh. No `tempfile` dependency exists in this
/// crate, so this follows the `tests/login_cli.rs` pattern.
#[allow(dead_code)]
fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ado-skills-cli-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
