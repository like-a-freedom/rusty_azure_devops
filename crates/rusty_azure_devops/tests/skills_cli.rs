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
