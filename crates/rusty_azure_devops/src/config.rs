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
        let yaml = Self::load_yaml().unwrap_or_default();

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

    /// Apply CA certificate settings. Call before creating HTTP clients.
    /// The Azure SDK reads `SSL_CERT_FILE` from the environment — no API exists to inject
    /// a custom CA directly into the SDK's HTTP client.
    pub fn apply_ca_cert(&self) {
        if let Some(ref cert_path) = self.ado_ca_cert
            && std::env::var("SSL_CERT_FILE").is_err()
        {
            // SAFETY: called before threads are spawned (both callers invoke this
            // immediately after Config::load and before AzureSdkClient::new).
            // The SDK has no API to inject a custom CA into its HTTP client.
            #[allow(unused_unsafe)]
            unsafe {
                std::env::set_var("SSL_CERT_FILE", cert_path)
            };
        }
    }

    fn load_yaml() -> Option<YamlConfig> {
        // Try binary-dir config first, then ~/.config/rusty_azure_devops/config.yaml
        if let Some(path) = Self::config_path()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(yaml) = serde_yaml::from_str(&content)
        {
            return Some(yaml);
        }
        Self::config_path_home().and_then(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            serde_yaml::from_str(&content).ok()
        })
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

/// Write `cfg` as YAML to an explicit `path`, creating parent dirs. The file
/// is created 0600 on Unix (PAT is a secret) and perms are re-asserted after
/// the write so a pre-existing 0644 file is tightened too.
pub(crate) fn save_to(path: &std::path::Path, cfg: &YamlConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create config dir {}: {}", parent.display(), e))?;
    }
    let content =
        serde_yaml::to_string(cfg).map_err(|e| format!("cannot serialize config: {}", e))?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Mode at creation: never leave a 0644 window on disk.
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("cannot chmod 0600 {}: {}", path.display(), e))?;
    }
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
    fn config_apply_ca_cert_no_cert_is_noop() {
        let c = Config {
            ado_token: "x".into(),
            ado_base_url: None,
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        c.apply_ca_cert();
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
}
