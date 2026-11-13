//! The shipped skills make factual claims about the CLI. These tests pin
//! those claims to the implementation, so a stale skill fails the build
//! instead of quietly misleading an agent.
//!
//! The command map is guarded separately, in `schema_validation.rs`.

use rusty_azure_devops::CommandError;
use rusty_azure_devops::domain::{GetWorkItemResult, WorkItemFields};
use rusty_azure_devops::skills::{SkillCatalog, catalog, embedded_catalog};

/// The skill catalog, read from the binary rather than from disk, so the
/// assertions cover what actually ships.
fn skills() -> SkillCatalog {
    embedded_catalog().expect("the embedded catalog assembles")
}

fn core_reference(name: &str) -> String {
    let catalog = skills();
    let core = catalog.get("core").expect("core skill");
    core.files
        .iter()
        .find(|f| f.path == name)
        .unwrap_or_else(|| panic!("core has {name}"))
        .content
        .clone()
}

fn on_prem_skill() -> String {
    skills()
        .get("on-prem")
        .expect("on-prem skill")
        .content
        .clone()
}

fn on_prem_reference(name: &str) -> String {
    let catalog = skills();
    let on_prem = catalog.get("on-prem").expect("on-prem skill");
    on_prem
        .files
        .iter()
        .find(|f| f.path == name)
        .unwrap_or_else(|| panic!("on-prem has {name}"))
        .content
        .clone()
}

/// The reason tokens the skill lists must all be real tokens the CLI can
/// emit, and each must exit with the code the skill's table promises.
#[test]
fn documented_reason_tokens_match_the_error_envelope() {
    let commands = core_reference("references/commands.md");

    // The token list, as the skill states it.
    let listed: Vec<String> = commands
        .split("stable token:")
        .nth(1)
        .expect("the skill lists the stable reason tokens")
        .split(".")
        .next()
        .expect("the token list ends at a sentence")
        .split(',')
        .filter_map(|token| token.split('`').nth(1))
        .map(str::to_string)
        .collect();
    assert!(
        listed.len() >= 7,
        "expected the skill to list every reason token, got {listed:?}"
    );

    for token in &listed {
        // `CommandError` stores its reason as a `&'static str`, so the
        // token read out of the skill has to outlive the call. Leaking it
        // is fine for a test.
        let reason: &'static str = Box::leak(token.clone().into_boxed_str());
        let error = CommandError::new(reason, String::new(), String::new());
        let expected = if token == "confirmation_required" {
            3
        } else {
            1
        };
        assert_eq!(
            error.exit_code, expected,
            "the skill must state the right exit code for {token}"
        );
    }

    // The two tokens the skills module itself can emit.
    assert!(listed.iter().any(|t| t == "skill_not_found"), "{listed:?}");
    assert!(
        listed.iter().any(|t| t == "reference_not_found"),
        "{listed:?}"
    );

    // The table must agree with the code about the confirmation code.
    assert!(
        commands.contains("| 3 | Confirmation required"),
        "the exit-code table must name the confirmation code"
    );
}

/// The jq path in the `System.Description` round-trip recipe must resolve
/// against a real payload, or the recipe writes a literal "null" into the
/// work item.
#[test]
fn the_documented_description_path_resolves() {
    let fields = core_reference("references/fields.md");

    let jq_line = fields
        .lines()
        .find(|line| line.contains("jq -r"))
        .expect("the round-trip recipe uses jq");
    let path = jq_line
        .split("jq -r '")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())
        .expect("the jq invocation names a path");

    let payload = serde_json::to_value(GetWorkItemResult {
        id: 42,
        rev: Some(1),
        fields: WorkItemFields {
            title: Some("T".to_string()),
            description: Some("D".to_string()),
            extra: None,
        },
        comments: None,
        relations: None,
    })
    .expect("the result serializes");

    let resolved = resolve_json_path(&payload, path);
    assert_eq!(
        resolved,
        Some(serde_json::json!("D")),
        "the documented path {path:?} does not resolve against the real payload: {payload}"
    );
}

/// Resolve a dotted path of `.key` and `["key"]` segments against a JSON
/// value. Enough for the paths the skills document.
fn resolve_json_path(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = value;
    for segment in parse_path(path) {
        current = current.get(&segment)?;
    }
    Some(current.clone())
}

/// Split a path into its segments. `.fields["System.Description"]` yields
/// `["fields", "System.Description"]`.
fn parse_path(path: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut chars = path.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            '.' => {
                chars.next();
                let mut key = String::new();
                while let Some(&n) = chars.peek() {
                    if n == '.' || n == '[' {
                        break;
                    }
                    key.push(n);
                    chars.next();
                }
                if !key.is_empty() {
                    segments.push(key);
                }
            }
            '[' => {
                chars.next();
                // Skip the opening quote, then read up to the closing one.
                if chars.peek() == Some(&'"') {
                    chars.next();
                }
                let mut key = String::new();
                while let Some(&n) = chars.peek() {
                    if n == '"' {
                        chars.next();
                        break;
                    }
                    key.push(n);
                    chars.next();
                }
                if chars.peek() == Some(&']') {
                    chars.next();
                }
                segments.push(key);
            }
            _ => {
                chars.next();
            }
        }
    }
    segments
}

/// The on-prem skills must attribute the dropped `fields` parameter to the
/// client, which is what drops it. The server rejects the combination; the
/// client removes the parameter before sending.
#[test]
fn the_on_prem_skill_attributes_the_field_drop_to_the_client() {
    for text in [
        on_prem_skill(),
        on_prem_reference("references/limitations.md"),
    ] {
        let flat = text.replace('\n', " ");
        let sentence = flat
            .split(". ")
            .find(|s| s.contains("drops `fields`"))
            .unwrap_or_else(|| panic!("a sentence says the fields parameter is dropped: {text}"));
        assert!(
            sentence.contains("client"),
            "the skill must say the client drops `fields`: {sentence}"
        );
    }
}

/// The skill must not promise a curated field set when the client drops the
/// parameter: the server then returns every field.
#[test]
fn the_on_prem_skill_does_not_promise_a_default_field_set() {
    for text in [
        on_prem_skill(),
        on_prem_reference("references/limitations.md"),
    ] {
        assert!(
            !text.contains("default field set"),
            "the client drops the fields parameter, so no curated set comes back: {text}"
        );
    }
}

/// Only `add-comment` falls back to `System.History`. `comments` has no
/// fallback and reports the failure.
#[test]
fn the_history_fallback_is_attributed_to_add_comment_only() {
    let commands = core_reference("references/commands.md");
    let section = commands
        .split("### `ado-cli comments` and `ado-cli add-comment`")
        .nth(1)
        .expect("the skill documents both comment commands")
        .split("### ")
        .next()
        .expect("the section ends at the next heading");
    assert!(
        section.contains("`add-comment` falls back"),
        "the fallback belongs to add-comment: {section}"
    );
    assert!(
        section.contains("`comments` has no such fallback"),
        "the skill must say that comments has no fallback: {section}"
    );
}

/// The skill's `login` claims must match the login flow.
#[test]
fn the_documented_login_flags_exist() {
    let commands = core_reference("references/commands.md");
    for flag in ["--token", "--token-stdin", "--force"] {
        assert!(
            commands.contains(flag),
            "the skill documents {flag}, which must exist"
        );
    }
    // The flags the skill names must be flags the CLI actually has.
    let help = assert_cmd::Command::cargo_bin("ado-cli")
        .unwrap()
        .args(["login", "--help"])
        .output()
        .expect("ado-cli runs");
    let help = String::from_utf8_lossy(&help.stdout);
    for flag in ["--token", "--token-stdin", "--force"] {
        assert!(help.contains(flag), "ado-cli login must accept {flag}");
    }
}

/// The catalog must serve the same skills the CLI advertises.
#[test]
fn the_advertised_skills_are_the_ones_that_ship() {
    let catalog = catalog().expect("the catalog assembles");
    let names: Vec<&str> = catalog.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["core", "on-prem"]);
}
