use std::env;
use std::io::Write;

const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub(crate) struct YamlConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_api_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_default_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ado_ca_cert: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub ado_token: String,
    pub ado_base_url: Option<String>,
    pub ado_organization: Option<String>,
    pub ado_api_version: Option<String>,
    pub ado_default_project: Option<String>,
    pub ado_ca_cert: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        Self::load_with_overrides(None, None)
    }

    pub fn load_with_overrides(
        token_override: Option<&str>,
        base_url_override: Option<&str>,
    ) -> Result<Self, String> {
        let yaml = Self::load_yaml_from_paths(
            Self::config_path().as_deref(),
            Self::config_path_home().as_deref(),
        )?
        .unwrap_or_default();

        let token = if let Some(t) = token_override {
            t.to_string()
        } else if let Ok(t) = env::var("ADO_TOKEN") {
            t
        } else if let Some(t) = yaml.ado_token {
            t
        } else {
            return Err(
                "missing ADO_TOKEN: set via --token flag, ADO_TOKEN env var, or config.yaml"
                    .to_string(),
            );
        };

        let base = if let Some(b) = base_url_override {
            Some(b.to_string())
        } else if let Ok(b) = env::var("ADO_BASE_URL") {
            Some(b)
        } else if let Some(b) = yaml.ado_base_url {
            Some(b)
        } else {
            return Err(
                "missing ADO_BASE_URL: set via --base-url flag, ADO_BASE_URL env var, or config.yaml"
                    .to_string(),
            );
        };

        let org = env::var("ADO_ORGANIZATION").ok().or(yaml.ado_organization);
        let api = env::var("ADO_API_VERSION").ok().or(yaml.ado_api_version);
        let def = env::var("ADO_DEFAULT_PROJECT")
            .ok()
            .or(yaml.ado_default_project);
        let ca_cert = env::var("ADO_CA_CERT").ok().or(yaml.ado_ca_cert);

        Ok(Self {
            ado_token: token,
            ado_base_url: base,
            ado_organization: org,
            ado_api_version: api,
            ado_default_project: def,
            ado_ca_cert: ca_cert,
        })
    }

    fn load_yaml_from_paths(
        binary_path: Option<&std::path::Path>,
        home_path: Option<&std::path::Path>,
    ) -> Result<Option<YamlConfig>, String> {
        for path in [binary_path, home_path].into_iter().flatten() {
            if let Some(cfg) = Self::load_yaml_file(path)? {
                return Ok(Some(cfg));
            }
        }
        Ok(None)
    }

    fn load_yaml_file(path: &std::path::Path) -> Result<Option<YamlConfig>, String> {
        Self::reject_insecure_config_file(path)?;
        let Ok(content) = std::fs::read_to_string(path) else {
            return Ok(None);
        };
        Ok(serde_yaml::from_str(&content).ok())
    }

    /// Refuse to load a config file that any group member or other user can
    /// write: such a file can silently redirect the PAT to a hostile endpoint.
    #[cfg(unix)]
    fn reject_insecure_config_file(path: &std::path::Path) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let Ok(metadata) = std::fs::metadata(path) else {
            return Ok(()); // absent file: caller's read step handles it
        };
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(format!(
                "refusing to read config {}: file is group/world-writable; \
                 set tighter permissions (e.g. chmod 600) or move it to a private directory",
                path.display()
            ));
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn reject_insecure_config_file(_path: &std::path::Path) -> Result<(), String> {
        Ok(())
    }

    fn config_path() -> Option<std::path::PathBuf> {
        let exe_dir = std::env::current_exe().ok()?;
        let dir = exe_dir.parent()?;
        Some(dir.join(CONFIG_FILENAME))
    }

    fn config_path_home() -> Option<std::path::PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        Some(
            std::path::PathBuf::from(home)
                .join(".config")
                .join("rusty_azure_devops")
                .join(CONFIG_FILENAME),
        )
    }
}

/// User-level config path: ~/.config/rusty_azure_devops/config.yaml
/// (login always writes here — never next to the binary).
pub(crate) fn user_config_path() -> Option<std::path::PathBuf> {
    Config::config_path_home()
}

/// Binary-dir config path (next to the executable). Exposed read-only so
/// login/logout can report that a deployed config shadows the user file.
pub(crate) fn binary_config_path() -> Option<std::path::PathBuf> {
    Config::config_path()
}

/// Create the config parent directory chain owner-only (0700) so a PAT can
/// never be published into a group-readable tree. Existing directories are
/// left untouched — their ACLs belong to whoever created them.
#[cfg(unix)]
fn create_config_dir(parent: &std::path::Path) -> std::io::Result<()> {
    use std::io::ErrorKind;
    use std::os::unix::fs::DirBuilderExt;
    // Collect the missing ancestors below the deepest existing one, then
    // create each with mode 0700 (DirBuilder::recursive would apply the mode
    // only to the leaf).
    let mut uncreated = Vec::new();
    let mut cursor = parent;
    while !cursor.exists() {
        uncreated.push(cursor.to_path_buf());
        match cursor.parent() {
            Some(p) => cursor = p,
            None => break,
        }
    }
    for dir in uncreated.iter().rev() {
        if let Err(e) = std::fs::DirBuilder::new().mode(0o700).create(dir)
            && (e.kind() != ErrorKind::AlreadyExists || !dir.is_dir())
        {
            return Err(e);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_config_dir(parent: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(parent)
}

/// Write `cfg` as YAML to an explicit `path`, creating parent dirs. On Unix,
/// tighten an existing file to 0600 before truncating or writing its contents.
pub(crate) fn save_to(path: &std::path::Path, cfg: &YamlConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        create_config_dir(parent)
            .map_err(|e| format!("cannot create config dir {}: {}", parent.display(), e))?;
    }
    let content =
        serde_yaml::to_string(cfg).map_err(|e| format!("cannot serialize config: {}", e))?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Mode at creation: never leave a 0644 window on disk.
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("cannot open config {}: {}", path.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("cannot chmod 0600 {}: {}", path.display(), e))?;
    }
    f.set_len(0)
        .map_err(|e| format!("cannot truncate config {}: {}", path.display(), e))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

/// Serialize `cfg` to the user config path. Overwrites unconditionally —
/// the caller (login) owns the --force guard.
pub(crate) fn save_user_config(cfg: &YamlConfig) -> Result<std::path::PathBuf, String> {
    let path = Config::config_path_home()
        .ok_or_else(|| "cannot resolve home directory for config save".to_string())?;
    save_to(&path, cfg)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_load_with_token_override() {
        let result = Config::load_with_overrides(Some("my_token"), Some("https://example.com"));
        assert!(result.is_ok());
        let c = result.unwrap();
        assert_eq!(c.ado_token, "my_token");
        assert_eq!(c.ado_base_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn create_modules_applies_custom_ca_without_mutating_ssl_cert_file() {
        if std::env::var_os("ADO_CA_CERT_PROBE").is_some() {
            let dir = std::env::temp_dir().join(format!("ado-ca-probe-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let ca_path = dir.join("synthetic-ca.pem");
            std::fs::write(&ca_path, include_str!("../tests/fixtures/synthetic-ca.pem")).unwrap();
            let config = Config {
                ado_token: "synthetic-token".into(),
                ado_base_url: Some("https://tfs.example.invalid/tfs/DefaultCollection".into()),
                ado_organization: None,
                ado_api_version: None,
                ado_default_project: None,
                ado_ca_cert: Some(ca_path.to_string_lossy().into_owned()),
            };
            let result = crate::create_modules(&config);
            let ssl_cert_file_is_unset = std::env::var_os("SSL_CERT_FILE").is_none();
            std::fs::remove_dir_all(&dir).ok();
            assert!(
                ssl_cert_file_is_unset,
                "custom CA configuration must not mutate the process environment"
            );
            assert!(result.is_ok(), "custom CA should configure the HTTP client");
            return;
        }

        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("config::tests::create_modules_applies_custom_ca_without_mutating_ssl_cert_file")
            .arg("--test-threads=1")
            .env("ADO_CA_CERT_PROBE", "1")
            .env_remove("SSL_CERT_FILE")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated CA configuration child failed (stdout: {}; stderr: {})",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn config_load_with_defaults_and_override_works() {
        let result = Config::load_with_overrides(Some("tok"), Some("https://x.com"));
        assert!(result.is_ok());
        let c = result.unwrap();
        assert_eq!(c.ado_token, "tok");
        assert_eq!(c.ado_base_url.as_deref(), Some("https://x.com"));
    }

    #[test]
    fn save_to_roundtrip_with_owner_only_perms() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        let cfg = crate::config::YamlConfig {
            ado_token: Some("pat123".into()),
            ado_base_url: Some("https://tfs.example.com/tfs/DefaultCollection".into()),
            ado_organization: None,
            ado_api_version: Some("6.0".into()),
            ado_default_project: Some("Demo".into()),
            ado_ca_cert: None,
        };
        crate::config::save_to(&path, &cfg).expect("save works");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("ado_base_url"));
        assert!(content.contains("pat123"));
        // Absent optionals are omitted, not written as `null` (skip_serializing_if).
        assert!(!content.contains("null"));
        // Roundtrip: what we wrote parses back through the real loader path.
        let parsed: crate::config::YamlConfig = serde_yaml::from_str(&content).unwrap();
        assert_eq!(
            parsed.ado_base_url.as_deref(),
            Some("https://tfs.example.com/tfs/DefaultCollection")
        );
        assert!(parsed.ado_organization.is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "PAT file must be owner-only");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_to_reasserts_perms_on_preexisting_loose_file() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-loose-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        std::fs::write(&path, "ado_token: old\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let cfg = crate::config::YamlConfig {
            ado_token: Some("new-pat".into()),
            ado_base_url: Some("https://tfs.example.com".into()),
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        crate::config::save_to(&path, &cfg).expect("save over existing");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "pre-existing 0644 file must be tightened");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    fn write_loose_yaml(dir: &std::path::Path, name: &str, token: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("ado_token: {token}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn load_yaml_rejects_group_or_world_writable_binary_config() {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-untrusted-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let binary_cfg = write_loose_yaml(&dir, "config.yaml", "attacker-endpoint-token");

        let result = Config::load_yaml_from_paths(Some(&binary_cfg), None);
        assert!(
            matches!(&result, Err(msg) if msg.contains("writable")),
            "group/world-writable config must be rejected with an explanatory error: {result:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn load_yaml_rejects_world_writable_home_config() {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-home-untrusted-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let home_cfg = write_loose_yaml(&dir, "home.yaml", "loose-token");

        let result = Config::load_yaml_from_paths(None, Some(&home_cfg));
        assert!(
            matches!(&result, Err(msg) if msg.contains("writable")),
            "world-writable home config must be rejected: {result:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn load_yaml_accepts_owner_only_binary_config() {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-trusted-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let binary_cfg = dir.join("config.yaml");
        std::fs::write(&binary_cfg, "ado_token: private-token\n").unwrap();
        std::fs::set_permissions(&binary_cfg, std::fs::Permissions::from_mode(0o600)).unwrap();

        let result = Config::load_yaml_from_paths(Some(&binary_cfg), None)
            .expect("owner-only config must load");
        assert_eq!(result.unwrap().ado_token.as_deref(), Some("private-token"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn save_to_creates_missing_parent_dirs_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-newdir-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let parent = dir.join(".config").join("rusty_azure_devops");
        let path = parent.join("config.yaml");
        let cfg = YamlConfig {
            ado_token: Some("secret".into()),
            ado_base_url: Some("https://tfs.example.invalid".into()),
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        save_to(&path, &cfg).expect("save with missing parents");
        let mode = std::fs::metadata(&parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "created config dir must be owner-only");
        let top_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(top_mode, 0o700, "every created dir must be owner-only");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn save_to_leaves_existing_parent_dir_mode_untouched() {
        use std::os::unix::fs::PermissionsExt;
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-existdir-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.join("config.yaml");
        let cfg = YamlConfig {
            ado_token: Some("secret".into()),
            ado_base_url: Some("https://tfs.example.invalid".into()),
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        save_to(&path, &cfg).expect("save into existing dir");
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "caller-owned existing dirs keep their mode");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn save_to_restricts_loose_file_before_publishing_new_token() {
        use std::io::Read;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{
            Arc, Barrier,
            atomic::{AtomicBool, Ordering},
        };
        use std::thread;

        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ado-cfg-window-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        std::fs::write(&path, "existing non-secret config\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let marker = "synthetic-window-token";
        let cfg = YamlConfig {
            ado_token: Some(format!("{marker}{}", "x".repeat(32 * 1024 * 1024))),
            ado_base_url: Some("https://tfs.example.invalid".into()),
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        let writing = Arc::new(AtomicBool::new(true));
        let observed = Arc::new(AtomicBool::new(false));
        let barrier = Arc::new(Barrier::new(2));
        let reader_path = path.clone();
        let reader_writing = Arc::clone(&writing);
        let reader_observed = Arc::clone(&observed);
        let reader_barrier = Arc::clone(&barrier);
        let reader = thread::spawn(move || {
            reader_barrier.wait();
            while reader_writing.load(Ordering::Acquire) {
                if let Ok(mut file) = std::fs::File::open(&reader_path)
                    && let Ok(metadata) = file.metadata()
                    && metadata.permissions().mode() & 0o077 != 0
                {
                    let mut prefix = [0; 128];
                    if let Ok(read) = file.read(&mut prefix)
                        && prefix[..read]
                            .windows(marker.len())
                            .any(|window| window == marker.as_bytes())
                    {
                        reader_observed.store(true, Ordering::Release);
                        break;
                    }
                }
            }
        });

        barrier.wait();
        let save_result = save_to(&path, &cfg);
        writing.store(false, Ordering::Release);
        reader.join().unwrap();
        save_result.unwrap();

        assert!(
            !observed.load(Ordering::Acquire),
            "new token bytes became readable while the config file had group/other permissions"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
