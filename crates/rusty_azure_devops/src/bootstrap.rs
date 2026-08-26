use azure_devops_client::AzureSdkClient;
use std::sync::Arc;

use crate::{
    CommentOperations, LinkOperations, ProjectLookup, QueryOperations, ServiceConfig,
    WorkItemOperations, config::Config,
};

/// Pre-built operation modules assembled from a shared client and config.
/// Constructed once at startup; handlers and the CLI receive them as a group.
pub struct Modules {
    pub work_items: WorkItemOperations,
    pub links: LinkOperations,
    pub comments: CommentOperations,
    pub projects: ProjectLookup,
    pub queries: QueryOperations,
    pub client: Arc<dyn azure_devops_client::AzureDevOpsClient>,
    pub service_config: ServiceConfig,
}

/// Build the shared `Modules` from a Config.
pub fn create_modules(config: &Config) -> Result<Modules, String> {
    let base = config
        .ado_base_url
        .clone()
        .ok_or_else(|| "ADO_BASE_URL required".to_string())?;
    let token = config.ado_token.clone();

    config.apply_ca_cert();

    let mut builder = AzureSdkClient::builder(
        base.clone(),
        token,
        config.ado_organization.clone().unwrap_or_default(),
    );
    if let Some(ref p) = config.ado_default_project {
        builder = builder.project(p);
    }
    if let Some(ref v) = config.ado_api_version {
        builder = builder.api_version(v);
    }
    let client = Arc::new(builder.build()) as Arc<dyn azure_devops_client::AzureDevOpsClient>;

    let service_config = ServiceConfig {
        base_url: Some(base),
        default_project: config.ado_default_project.clone(),
        token_present: !config.ado_token.is_empty(),
    };

    Ok(Modules {
        work_items: WorkItemOperations::with_config(client.clone(), service_config.clone()),
        links: LinkOperations::with_config(client.clone(), service_config.clone()),
        comments: CommentOperations::new(client.clone()),
        projects: ProjectLookup::with_config(client.clone(), service_config.clone()),
        queries: QueryOperations::new(client.clone()),
        client,
        service_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn modules_exposes_all_operations() {
        let config = Config {
            ado_token: "test-token".into(),
            ado_base_url: Some("https://tfs.example.com".into()),
            ado_organization: None,
            ado_api_version: Some("6.0".into()),
            ado_default_project: Some("Demo".into()),
            ado_ca_cert: None,
        };
        let modules = create_modules(&config).unwrap();
        assert_eq!(
            modules.service_config.default_project.as_deref(),
            Some("Demo")
        );
        assert!(modules.service_config.token_present);

        // Verify all modules are accessible via struct fields
        let _: WorkItemOperations = modules.work_items;
        let _: ProjectLookup = modules.projects;
        let _: QueryOperations = modules.queries;
        let _: LinkOperations = modules.links;
        let _: CommentOperations = modules.comments;
    }

    #[test]
    fn create_modules_fails_without_base_url() {
        let config = Config {
            ado_token: "token".into(),
            ado_base_url: None,
            ado_organization: None,
            ado_api_version: None,
            ado_default_project: None,
            ado_ca_cert: None,
        };
        assert!(create_modules(&config).is_err());
    }
}
