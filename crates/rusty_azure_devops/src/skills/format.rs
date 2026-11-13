//! Rendering the skill catalog for humans and for machines.
//!
//! Every function here **returns** a value; none of them print. The CLI
//! prints the result. That keeps the human form byte-exact and makes both
//! forms unit-testable.
//!
//! The JSON DTOs live here rather than on the domain types: the domain
//! `SkillFile` always carries its content, but `skills list` omits it. A
//! DTO with `Option<String>` makes that omission explicit — deriving it
//! from the domain type with `skip_serializing_if = "String::is_empty"`
//! would silently drop the content of a legitimately empty reference file
//! in `skills get --full` too.

use serde::Serialize;

use super::{CatalogLocation, Skill, SkillCatalog, SkillFile};

/// One file of a skill, as it appears in JSON.
///
/// `content` is absent when only the path is listed.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    /// Path relative to the skill root, using `/` separators.
    pub path: String,
    /// The file's bytes, when the listing carries them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// One skill, as it appears in `skills list`.
#[derive(Debug, Clone, Serialize)]
pub struct SkillListEntry {
    pub name: String,
    pub description: String,
    pub files: Vec<FileEntry>,
}

/// The JSON envelope for `skills get` and `skills get --full`.
///
/// `files` is absent for the plain `get` envelope and present for `--full`.
/// The struct is what actually gets serialized, so a new field cannot be
/// added here and then forgotten in the output.
#[derive(Debug, Clone, Serialize)]
pub struct SkillEnvelope {
    pub ok: bool,
    pub name: String,
    pub description: String,
    /// The whole SKILL.md, verbatim, including its frontmatter.
    pub content: String,
    /// The skill's supporting files, with their content. Absent unless the
    /// caller asked for them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileEntry>>,
}

/// A file entry carrying its content.
fn file_with_content(file: &SkillFile) -> FileEntry {
    FileEntry {
        path: file.path.clone(),
        content: Some(file.content.clone()),
    }
}

/// A file entry carrying only its path.
fn file_path_only(file: &SkillFile) -> FileEntry {
    FileEntry {
        path: file.path.clone(),
        content: None,
    }
}

/// The JSON envelope for `skills list`.
pub fn skill_list_json(catalog: &SkillCatalog) -> serde_json::Value {
    let skills: Vec<SkillListEntry> = catalog
        .skills
        .iter()
        .map(|skill| SkillListEntry {
            name: skill.name.clone(),
            description: skill.description.clone(),
            files: skill.files.iter().map(file_path_only).collect(),
        })
        .collect();
    serde_json::json!({ "ok": true, "skills": skills })
}

/// The human form of `skills list`: one line per skill, then its file paths.
pub fn skill_list_human(catalog: &SkillCatalog) -> String {
    let width = catalog
        .skills
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for skill in &catalog.skills {
        out.push_str(&format!(
            "{:<width$}  {}\n",
            skill.name,
            skill.description,
            width = width
        ));
    }
    for skill in &catalog.skills {
        for file in &skill.files {
            out.push_str(&format!("  {}: {}\n", skill.name, file.path));
        }
    }
    out
}

/// The JSON envelope for `skills get`: the skill alone, without its
/// supporting files.
pub fn skill_detail_json(skill: &Skill) -> serde_json::Value {
    let envelope = SkillEnvelope {
        ok: true,
        name: skill.name.clone(),
        description: skill.description.clone(),
        content: skill.content.clone(),
        files: None,
    };
    serde_json::to_value(envelope).expect("a SkillEnvelope always serializes")
}

/// The JSON envelope for `skills get --full`: the skill plus every supporting
/// file and its content.
pub fn skill_full_json(skill: &Skill) -> serde_json::Value {
    let envelope = SkillEnvelope {
        ok: true,
        name: skill.name.clone(),
        description: skill.description.clone(),
        content: skill.content.clone(),
        files: Some(skill.files.iter().map(file_with_content).collect()),
    };
    serde_json::to_value(envelope).expect("a SkillEnvelope always serializes")
}

/// The JSON envelope for `skills get --ref <path>`: one supporting file.
pub fn skill_file_json(skill: &Skill, file: &SkillFile) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "name": skill.name,
        "path": file.path,
        "content": file.content,
    })
}

/// The human form of `skills get`: the SKILL.md, byte for byte.
pub fn skill_detail_human(skill: &Skill) -> String {
    skill.content.clone()
}

/// The human form of `skills get --full`: the SKILL.md, then each file
/// preceded by a marker line naming its exact path.
///
/// The marker path is the value `--ref` accepts, so an agent can copy it
/// straight out of the output.
pub fn skill_full_human(skill: &Skill) -> String {
    let mut out = skill.content.clone();
    for file in &skill.files {
        out.push_str(&format!("\n--- {} ---\n", file.path));
        out.push_str(&file.content);
    }
    out
}

/// The human form of `skills get --ref <path>`: the file, byte for byte.
pub fn skill_file_human(_skill: &Skill, file: &SkillFile) -> String {
    file.content.clone()
}

/// The JSON envelope for `skills path`.
pub fn catalog_location_json(
    location: &CatalogLocation,
    available: &[String],
) -> serde_json::Value {
    match location {
        CatalogLocation::Embedded => serde_json::json!({
            "ok": true,
            "location": "embedded",
            "available": available,
        }),
        CatalogLocation::Directory(path) => serde_json::json!({
            "ok": true,
            "location": "directory",
            "path": path.to_string_lossy(),
            "available": available,
        }),
    }
}

/// The human form of `skills path`.
pub fn catalog_location_human(location: &CatalogLocation, available: &[String]) -> String {
    let mut out = match location {
        CatalogLocation::Embedded => "embedded".to_string(),
        CatalogLocation::Directory(path) => format!("directory: {}", path.display()),
    };
    out.push('\n');
    out.push_str(&format!("available: {}", available.join(", ")));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{CatalogLocation, Skill, SkillCatalog, SkillFile};

    fn sample() -> SkillCatalog {
        SkillCatalog {
            skills: vec![Skill {
                name: "core".into(),
                description: "Core usage.".into(),
                content: "---\nname: core\n---\nbody\n".into(),
                files: vec![SkillFile {
                    path: "references/commands.md".into(),
                    content: "ado-cli status\n".into(),
                }],
            }],
            location: CatalogLocation::Embedded,
        }
    }

    #[test]
    fn skill_list_json_omits_content_but_keeps_path() {
        let v = skill_list_json(&sample());
        assert_eq!(v["ok"], serde_json::json!(true));
        assert_eq!(v["skills"][0]["name"], serde_json::json!("core"));
        assert_eq!(
            v["skills"][0]["files"][0]["path"],
            serde_json::json!("references/commands.md")
        );
        assert!(
            v["skills"][0]["files"][0].get("content").is_none(),
            "list must not carry file content"
        );
    }

    #[test]
    fn detail_json_omits_files_but_full_json_carries_them() {
        let skill = &sample().skills[0];
        let detail = skill_detail_json(skill);
        assert!(
            detail.get("files").is_none(),
            "the plain `get` envelope must not carry files: {detail}"
        );
        assert_eq!(detail["name"], serde_json::json!("core"));

        let full = skill_full_json(skill);
        assert_eq!(
            full["files"][0]["path"],
            serde_json::json!("references/commands.md")
        );
        assert_eq!(
            full["files"][0]["content"],
            serde_json::json!("ado-cli status\n")
        );
    }

    #[test]
    fn skill_full_json_carries_content() {
        let v = skill_full_json(&sample().skills[0]);
        assert!(v["content"].as_str().unwrap().contains("body"));
        assert_eq!(
            v["files"][0]["content"],
            serde_json::json!("ado-cli status\n")
        );
    }

    #[test]
    fn full_human_marks_each_file_with_its_exact_path() {
        let out = skill_full_human(&sample().skills[0]);
        assert!(out.contains("--- references/commands.md ---"), "{out}");
        assert!(out.contains("ado-cli status"), "{out}");
        assert!(
            out.find("--- references/commands.md ---").unwrap() > out.find("body").unwrap(),
            "{out}"
        );
    }

    #[test]
    fn detail_human_is_byte_exact_skill_content() {
        let out = skill_detail_human(&sample().skills[0]);
        assert_eq!(out, "---\nname: core\n---\nbody\n");
    }

    #[test]
    fn file_human_is_byte_exact_file_content() {
        let skill = &sample().skills[0];
        let out = skill_file_human(skill, &skill.files[0]);
        assert_eq!(out, "ado-cli status\n");
    }

    #[test]
    fn list_human_names_every_skill() {
        let out = skill_list_human(&sample());
        assert!(out.contains("core"), "{out}");
        assert!(out.contains("Core usage."), "{out}");
        assert!(out.contains("references/commands.md"), "{out}");
    }

    #[test]
    fn location_human_ends_with_a_newline() {
        let names = vec!["core".to_string()];
        let out = catalog_location_human(&CatalogLocation::Embedded, &names);
        assert!(
            out.ends_with('\n'),
            "output must end with a newline: {out:?}"
        );
    }

    #[test]
    fn location_json_reports_the_source_and_the_available_skills() {
        let names = vec!["core".to_string()];
        let embedded = catalog_location_json(&CatalogLocation::Embedded, &names);
        assert_eq!(embedded["location"], serde_json::json!("embedded"));
        assert_eq!(embedded["available"], serde_json::json!(["core"]));

        let directory = catalog_location_json(
            &CatalogLocation::Directory(std::path::PathBuf::from("/tmp/skills")),
            &names,
        );
        assert_eq!(directory["location"], serde_json::json!("directory"));
        assert_eq!(directory["path"], serde_json::json!("/tmp/skills"));
    }
}
