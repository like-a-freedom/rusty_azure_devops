use std::env;

const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct YamlConfig {
    ado_token: Option<String>,
    ado_base_url: Option<String>,
    ado_organization: Option<String>,
    ado_api_version: Option<String>,
    ado_default_project: Option<String>,
    ado_ca_cert: Option<String>,
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
}
