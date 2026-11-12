//! Agent-facing usage guides served by the CLI, embedded in the binary.
//!
//! A bounded context of its own: this module knows nothing about the Azure
//! DevOps domain (WorkItem, Relation, Patch Document) and depends on neither
//! `azure_devops_client` nor the operation layer. It reads content and
//! parses it — nothing more.
//!
//! See docs/adr/0017-cli-agent-skills.md.

use std::path::PathBuf;

/// One agent-facing usage guide: a `SKILL.md` plus its supporting files.
#[derive(Debug, Clone)]
pub struct Skill {
    /// The directory name — the addressing key.
    pub name: String,
    /// From the SKILL.md frontmatter.
    pub description: String,
    /// The whole SKILL.md file, verbatim, including its frontmatter.
    pub content: String,
    /// Supporting files, sorted by path. Never contains `SKILL.md`.
    pub files: Vec<SkillFile>,
}

/// A supporting file inside a skill directory.
#[derive(Debug, Clone)]
pub struct SkillFile {
    /// Path relative to the skill root, using `/` separators.
    pub path: String,
    pub content: String,
}

/// The set of skills the CLI serves.
#[derive(Debug, Clone)]
pub struct SkillCatalog {
    /// Valid skills, sorted by name.
    pub skills: Vec<Skill>,
    pub location: CatalogLocation,
}

/// Where the catalog's content came from.
#[derive(Debug, Clone)]
pub enum CatalogLocation {
    /// Compiled into this binary.
    Embedded,
    /// Read from a directory on disk.
    Directory(PathBuf),
}

/// Errors the skills domain raises. The CLI maps each variant onto a stable
/// reason token (see `impl From<SkillsError> for CommandError`).
#[derive(Debug, Clone)]
pub enum SkillsError {
    /// No skill with that name in the catalog.
    SkillNotFound { name: String },
    /// The path is not in the skill's file set.
    ReferenceNotFound { skill: String, path: String },
    /// A skill's frontmatter is missing, malformed, or disagrees with its
    /// directory name.
    Invalid { name: String, detail: String },
    /// The content source could not be read.
    Io { detail: String },
}

impl std::fmt::Display for SkillsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillsError::SkillNotFound { name } => write!(f, "no skill named '{name}'"),
            SkillsError::ReferenceNotFound { skill, path } => {
                write!(f, "'{path}' is not a file of skill '{skill}'")
            }
            SkillsError::Invalid { name, detail } => {
                write!(f, "skill '{name}' is unusable: {detail}")
            }
            SkillsError::Io { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for SkillsError {}

impl Skill {
    /// Look up a supporting file by its exact path, relative to the skill
    /// root. A leading `./` is accepted and ignored.
    ///
    /// Matching is exact: no file-name or stem fallback, so the lookup can
    /// never be ambiguous. There is no filesystem here, so `..` traversal
    /// is impossible.
    pub fn file(&self, path: &str) -> Result<&SkillFile, SkillsError> {
        let wanted = path.strip_prefix("./").unwrap_or(path);
        self.files
            .iter()
            .find(|file| file.path == wanted)
            .ok_or_else(|| SkillsError::ReferenceNotFound {
                skill: self.name.clone(),
                path: path.to_string(),
            })
    }
}

impl SkillCatalog {
    /// Find a skill by name.
    pub fn get(&self, name: &str) -> Result<&Skill, SkillsError> {
        self.skills
            .iter()
            .find(|skill| skill.name == name)
            .ok_or_else(|| SkillsError::SkillNotFound {
                name: name.to_string(),
            })
    }
}

/// Parse the YAML frontmatter of a `SKILL.md` file.
///
/// Returns `(name, description)`. Both are required by the Agent Skills
/// specification; extra frontmatter keys are ignored.
pub fn parse_frontmatter(content: &str) -> Result<(String, String), String> {
    let trimmed = content.trim_start();
    let rest = trimmed
        .strip_prefix("---")
        .ok_or_else(|| "missing frontmatter block".to_string())?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| "missing frontmatter block".to_string())?;
    let yaml = &rest[..end];

    let value: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("malformed frontmatter: {e}"))?;
    let mapping = value
        .as_mapping()
        .ok_or_else(|| "malformed frontmatter: expected a mapping".to_string())?;

    let field = |key: &str| -> Option<String> {
        mapping
            .get(serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };

    let name = field("name").ok_or_else(|| "frontmatter is missing 'name'".to_string())?;
    let description =
        field("description").ok_or_else(|| "frontmatter is missing 'description'".to_string())?;
    Ok((name, description))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\nname: core\ndescription: Core usage.\n---\n\n# Core\n\nbody\n";

    #[test]
    fn frontmatter_reads_name_and_description() {
        let (name, desc) = parse_frontmatter(GOOD).unwrap();
        assert_eq!(name, "core");
        assert_eq!(desc, "Core usage.");
    }

    #[test]
    fn frontmatter_rejects_missing_name() {
        let err = parse_frontmatter("---\ndescription: x\n---\nbody").unwrap_err();
        assert!(err.contains("name"), "{err}");
    }

    #[test]
    fn frontmatter_rejects_missing_description() {
        let err = parse_frontmatter("---\nname: core\n---\nbody").unwrap_err();
        assert!(err.contains("description"), "{err}");
    }

    #[test]
    fn frontmatter_rejects_malformed_yaml() {
        let err = parse_frontmatter("---\nname: [unclosed\n---\nbody").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn frontmatter_rejects_absent_block() {
        assert!(parse_frontmatter("# no frontmatter\n").is_err());
    }

    #[test]
    fn skill_file_lookup_is_exact_and_excludes_skill_md() {
        let skill = Skill {
            name: "core".into(),
            description: "d".into(),
            content: GOOD.into(),
            files: vec![SkillFile {
                path: "references/commands.md".into(),
                content: "c".into(),
            }],
        };
        assert_eq!(
            skill.file("references/commands.md").unwrap().path,
            "references/commands.md"
        );
        assert!(
            skill.file("./references/commands.md").is_ok(),
            "leading ./ is accepted"
        );
        assert!(
            skill.file("SKILL.md").is_err(),
            "SKILL.md is not in the file set"
        );
        assert!(skill.file("commands.md").is_err(), "no stem fallback");
        assert!(skill.file("../outside.md").is_err(), "no traversal");
    }

    #[test]
    fn catalog_get_reports_unknown_skill() {
        let catalog = SkillCatalog {
            skills: vec![],
            location: CatalogLocation::Embedded,
        };
        let err = catalog.get("nope").unwrap_err();
        assert!(
            matches!(err, SkillsError::SkillNotFound { ref name } if name == "nope"),
            "{err:?}"
        );
    }
}
