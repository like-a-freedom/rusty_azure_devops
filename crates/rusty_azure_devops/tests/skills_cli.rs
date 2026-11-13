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
fn skills_get_json_omits_files_and_full_json_carries_them() {
    let plain = ado_cli()
        .args(["skills", "get", "core", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plain: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    assert_eq!(plain["name"], serde_json::json!("core"));
    assert!(
        plain.get("files").is_none(),
        "the plain get envelope must not carry files: {plain}"
    );

    let full = ado_cli()
        .args(["skills", "get", "core", "--full", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let full: serde_json::Value = serde_json::from_slice(&full).unwrap();
    assert_eq!(
        full["files"][0]["path"],
        serde_json::json!("references/commands.md")
    );
    assert!(
        full["files"][0]["content"]
            .as_str()
            .unwrap()
            .contains("ado-cli get-work-item"),
        "full must carry file content: {full}"
    );
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

#[test]
fn override_dir_replaces_the_bundled_catalog() {
    let dir = temp_dir("override");
    std::fs::create_dir_all(dir.join("mine")).unwrap();
    std::fs::write(
        dir.join("mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine.\n---\nbody\n",
    )
    .unwrap();
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", &dir)
        .args(["skills", "path", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["location"], serde_json::json!("directory"));
    assert_eq!(v["available"], serde_json::json!(["mine"]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn override_dir_serves_its_own_skills() {
    let dir = temp_dir("serve");
    std::fs::create_dir_all(dir.join("mine")).unwrap();
    std::fs::write(
        dir.join("mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine.\n---\nbody\n",
    )
    .unwrap();
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", &dir)
        .args(["skills", "get", "mine", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["name"], serde_json::json!("mine"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn empty_override_is_treated_as_unset() {
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", "")
        .args(["skills", "path", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["location"], serde_json::json!("embedded"));
}

#[test]
fn missing_override_dir_is_an_io_error() {
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", "/definitely/not/here")
        .args(["skills", "path", "--json"])
        .assert()
        .failure()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], serde_json::json!("io_error"));
}

#[test]
fn malformed_override_skill_fails_the_whole_catalog() {
    let dir = temp_dir("malformed");
    std::fs::create_dir_all(dir.join("mine")).unwrap();
    std::fs::write(dir.join("mine/SKILL.md"), "no frontmatter here\n").unwrap();
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", &dir)
        .args(["skills", "list", "--json"])
        .assert()
        .failure()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["reason"], serde_json::json!("validation_failed"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn override_path_is_reported_as_absolute() {
    let dir = temp_dir("absolute");
    std::fs::create_dir_all(dir.join("mine")).unwrap();
    std::fs::write(
        dir.join("mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine.\n---\nbody\n",
    )
    .unwrap();
    let out = ado_cli()
        .env("ADO_CLI_SKILLS_DIR", &dir)
        .args(["skills", "path", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let path = v["path"].as_str().unwrap();
    assert!(path.starts_with('/'), "path must be absolute: {path}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A temp directory, created fresh. No `tempfile` dependency exists in this
/// crate, so this follows the `tests/login_cli.rs` pattern.
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
