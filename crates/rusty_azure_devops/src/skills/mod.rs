//! Agent-facing usage guides served by the CLI, embedded in the binary.
//!
//! A bounded context of its own: this module knows nothing about the Azure
//! DevOps domain (WorkItem, Relation, Patch Document) and depends on neither
//! `azure_devops_client` nor the operation layer. It reads content and
//! parses it — nothing more.
//!
//! See docs/adr/0017-cli-agent-skills.md.

pub mod source;

use std::path::PathBuf;

use source::{DirectorySource, EmbeddedSource, SkillSource};

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

/// The catalog the CLI serves: the embedded skills, unless
/// `ADO_CLI_SKILLS_DIR` replaces them.
///
/// An exported-but-empty variable is treated as unset, so a shell profile
/// that leaves `ADO_CLI_SKILLS_DIR=` empty does not break the CLI.
pub fn catalog() -> Result<SkillCatalog, SkillsError> {
    match std::env::var("ADO_CLI_SKILLS_DIR") {
        Ok(dir) if !dir.trim().is_empty() => {
            let source = DirectorySource::new(PathBuf::from(dir)).map_err(|e| match e {
                SkillsError::Io { detail } => SkillsError::Io {
                    detail: format!("ADO_CLI_SKILLS_DIR: {detail}"),
                },
                other => other,
            })?;
            let location = CatalogLocation::Directory(source.root.clone());
            build_catalog(&source, location)
        }
        _ => embedded_catalog(),
    }
}

/// The catalog compiled into this binary, ignoring `ADO_CLI_SKILLS_DIR`.
pub fn embedded_catalog() -> Result<SkillCatalog, SkillsError> {
    build_catalog(&EmbeddedSource, CatalogLocation::Embedded)
}

/// Assemble a catalog from a source.
///
/// Files are grouped by their first path segment — the skill directory
/// name. `SKILL.md` is the skill's content; every other file is a supporting
/// file, addressed relative to the skill root.
///
/// The whole catalog fails on the first unusable skill: there is no
/// partially-served catalog and no separate list of broken skills.
pub(crate) fn build_catalog(
    source: &dyn SkillSource,
    location: CatalogLocation,
) -> Result<SkillCatalog, SkillsError> {
    let files = source.files()?;

    let mut by_skill: std::collections::BTreeMap<String, Vec<(String, Vec<u8>)>> =
        std::collections::BTreeMap::new();
    for (path, bytes) in files {
        let Some((skill, rest)) = path.split_once('/') else {
            // A file at the source root is not inside a skill directory.
            continue;
        };
        by_skill
            .entry(skill.to_string())
            .or_default()
            .push((rest.to_string(), bytes));
    }

    let mut skills = Vec::with_capacity(by_skill.len());
    for (name, entries) in by_skill {
        let skill_md = entries
            .iter()
            .find(|(path, _)| path == "SKILL.md")
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .ok_or_else(|| SkillsError::Invalid {
                name: name.clone(),
                detail: "no SKILL.md".to_string(),
            })?;

        let (frontmatter_name, description) =
            parse_frontmatter(&skill_md).map_err(|detail| SkillsError::Invalid {
                name: name.clone(),
                detail,
            })?;
        if frontmatter_name != name {
            return Err(SkillsError::Invalid {
                name: name.clone(),
                detail: format!(
                    "frontmatter name '{frontmatter_name}' does not match directory '{name}'"
                ),
            });
        }

        let mut files: Vec<SkillFile> = entries
            .into_iter()
            .filter(|(path, _)| path != "SKILL.md")
            .map(|(path, bytes)| SkillFile {
                path,
                content: String::from_utf8_lossy(&bytes).into_owned(),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        skills.push(Skill {
            name,
            description,
            content: skill_md,
            files,
        });
    }

    Ok(SkillCatalog { skills, location })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A temp directory, created fresh. No `tempfile` dependency exists in
    /// this crate, so this follows the `tests/login_cli.rs` pattern.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ado-skills-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    #[test]
    fn embedded_catalog_yields_both_bundled_skills() {
        let catalog = embedded_catalog().expect("embedded catalog assembles");
        let names: Vec<&str> = catalog.skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["core", "on-prem"]);
        assert!(matches!(catalog.location, CatalogLocation::Embedded));
    }

    #[test]
    fn embedded_catalog_exposes_reference_files_relative_to_the_skill_root() {
        let catalog = embedded_catalog().expect("embedded catalog assembles");
        let core = catalog.get("core").unwrap();
        let paths: Vec<&str> = core.files.iter().map(|f| f.path.as_str()).collect();
        assert!(
            paths.contains(&"references/commands.md"),
            "paths are relative to the skill root: {paths:?}"
        );
    }

    #[test]
    fn catalog_fails_when_a_skill_frontmatter_is_unusable() {
        let dir = temp_dir("invalid");
        std::fs::create_dir_all(dir.join("mine")).unwrap();
        std::fs::write(dir.join("mine/SKILL.md"), "no frontmatter here\n").unwrap();
        let source = DirectorySource::new(dir.clone()).unwrap();
        let err = build_catalog(&source, CatalogLocation::Embedded).unwrap_err();
        assert!(
            matches!(err, SkillsError::Invalid { ref name, .. } if name == "mine"),
            "{err:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_fails_when_frontmatter_name_disagrees_with_the_directory() {
        let dir = temp_dir("mismatch");
        std::fs::create_dir_all(dir.join("mine")).unwrap();
        std::fs::write(
            dir.join("mine/SKILL.md"),
            "---\nname: other\ndescription: d\n---\nbody\n",
        )
        .unwrap();
        let source = DirectorySource::new(dir.clone()).unwrap();
        let err = build_catalog(&source, CatalogLocation::Embedded).unwrap_err();
        assert!(
            matches!(err, SkillsError::Invalid { ref name, .. } if name == "mine"),
            "{err:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn catalog_builds_skills_from_a_source() {
        let dir = temp_dir("build");
        std::fs::create_dir_all(dir.join("mine/references")).unwrap();
        std::fs::write(
            dir.join("mine/SKILL.md"),
            "---\nname: mine\ndescription: Mine.\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(dir.join("mine/references/commands.md"), "ado-cli status\n").unwrap();
        let source = DirectorySource::new(dir.clone()).unwrap();
        let catalog = build_catalog(&source, CatalogLocation::Embedded).unwrap();
        assert_eq!(catalog.skills.len(), 1);
        let skill = &catalog.skills[0];
        assert_eq!(skill.name, "mine");
        assert_eq!(skill.description, "Mine.");
        assert!(skill.content.contains("body"));
        assert_eq!(skill.files.len(), 1);
        assert_eq!(skill.files[0].path, "references/commands.md");
        assert_eq!(skill.files[0].content, "ado-cli status\n");
        std::fs::remove_dir_all(&dir).ok();
    }
}
