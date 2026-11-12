//! Where skill content comes from: the compiled-in tree or a directory on
//! disk. The seam that lets the rest of the module stay source-agnostic.

use std::path::{Path, PathBuf};

use super::SkillsError;

/// A source of skill files. `files()` returns `(relative path, bytes)` pairs;
/// the path uses `/` separators and is relative to the skill root.
pub trait SkillSource {
    fn files(&self) -> Result<Vec<(String, Vec<u8>)>, SkillsError>;
}

/// The skill tree compiled into this binary.
static EMBEDDED: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/skills");

/// The skill tree embedded in the binary.
pub struct EmbeddedSource;

impl SkillSource for EmbeddedSource {
    fn files(&self) -> Result<Vec<(String, Vec<u8>)>, SkillsError> {
        let mut out = Vec::new();
        collect_embedded(&EMBEDDED, "", &mut out);
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

/// A skill tree read from a directory on disk.
pub struct DirectorySource {
    /// The canonicalized root directory.
    pub root: PathBuf,
}

impl DirectorySource {
    /// Canonicalize `root` so the reported location is unambiguous.
    pub fn new(root: PathBuf) -> Result<Self, SkillsError> {
        let canonical = root.canonicalize().map_err(|e| SkillsError::Io {
            detail: format!("{}: {e}", root.display()),
        })?;
        if !canonical.is_dir() {
            return Err(SkillsError::Io {
                detail: format!("{}: not a directory", root.display()),
            });
        }
        Ok(Self { root: canonical })
    }
}

impl SkillSource for DirectorySource {
    fn files(&self) -> Result<Vec<(String, Vec<u8>)>, SkillsError> {
        let mut out = Vec::new();
        collect_dir(&self.root, &self.root, &mut out)?;
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}

/// True when any segment of `path` starts with `.`.
///
/// An operating system must not add content to a skill: a stray `.DS_Store`
/// would otherwise be embedded and served as part of a skill.
pub(crate) fn is_hidden(path: &str) -> bool {
    path.split('/').any(|segment| segment.starts_with('.'))
}

/// Walk the embedded tree, accumulating `(relative path, bytes)`.
fn collect_embedded(dir: &include_dir::Dir<'_>, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) {
    for entry in dir.entries() {
        let name = entry
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let relative = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry {
            include_dir::DirEntry::Dir(sub) => collect_embedded(sub, &relative, out),
            include_dir::DirEntry::File(file) => {
                if !is_hidden(&relative) {
                    out.push((relative, file.contents().to_vec()));
                }
            }
        }
    }
}

/// Walk a directory tree, accumulating `(relative path, bytes)`.
fn collect_dir(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), SkillsError> {
    let entries = std::fs::read_dir(dir).map_err(|e| SkillsError::Io {
        detail: format!("{}: {e}", dir.display()),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| SkillsError::Io {
            detail: format!("{}: {e}", dir.display()),
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if is_hidden(&relative) {
            continue;
        }
        if path.is_dir() {
            collect_dir(root, &path, out)?;
        } else {
            let bytes = std::fs::read(&path).map_err(|e| SkillsError::Io {
                detail: format!("{}: {e}", path.display()),
            })?;
            out.push((relative, bytes));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn write(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn directory_source_lists_nested_files() {
        let dir = temp_dir("nested");
        write(&dir, "core/SKILL.md", "body");
        write(&dir, "core/references/commands.md", "cmds");
        let source = DirectorySource::new(dir.clone()).unwrap();
        let files = source.files().unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, ["core/SKILL.md", "core/references/commands.md"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn directory_source_normalizes_separators_to_slash() {
        let dir = temp_dir("sep");
        write(&dir, "core/references/commands.md", "cmds");
        let source = DirectorySource::new(dir.clone()).unwrap();
        for (path, _) in source.files().unwrap() {
            assert!(!path.contains('\\'), "path must use / separators: {path}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn directory_source_rejects_missing_root() {
        let missing = std::env::temp_dir().join("ado-skills-does-not-exist-xyz");
        assert!(DirectorySource::new(missing).is_err());
    }

    #[test]
    fn directory_source_skips_dotfiles() {
        let dir = temp_dir("dot");
        write(&dir, "core/SKILL.md", "body");
        write(&dir, "core/.DS_Store", "junk");
        let source = DirectorySource::new(dir.clone()).unwrap();
        let paths: Vec<String> = source
            .files()
            .unwrap()
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        assert!(!paths.iter().any(|p| p.contains(".DS_Store")), "{paths:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn embedded_source_walks_nested_files_and_skips_dotfiles() {
        let files = EmbeddedSource.files().unwrap();
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(
            paths.contains(&"core/SKILL.md"),
            "embedded tree must carry core/SKILL.md: {paths:?}"
        );
        assert!(
            paths.contains(&"on-prem/SKILL.md"),
            "embedded tree must carry on-prem/SKILL.md: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.starts_with("core/references/")),
            "embedded tree must carry nested reference files: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains(".DS_Store")),
            "dotfiles must not be embedded: {paths:?}"
        );
    }
}
