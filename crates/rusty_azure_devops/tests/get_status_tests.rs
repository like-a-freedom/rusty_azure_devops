use azure_devops_client::InMemoryAzureClient;
use rusty_azure_devops::{GetStatusParams, ProjectLookup, ServiceConfig};
use std::sync::Arc;

#[tokio::test]
async fn get_status_with_show_secrets_returns_config() {
    let client = Arc::new(InMemoryAzureClient::default());
    let config = ServiceConfig {
        base_url: Some("https://dev.azure.com/testorg".into()),
        default_project: Some("Demo".into()),
        token_present: true,
    };
    let projects = ProjectLookup::with_config(client, config);

    let params = GetStatusParams {
        show_secrets: Some(true),
        include_projects: Some(true),
    };
    let res = projects.get_status(params).await.expect("get status ok");
    assert_eq!(res.status, "ok");
    assert_eq!(res.project_count, 1);
    assert!(!res.projects.is_empty());
    let cfg = res.config.unwrap();
    assert!(cfg.token_present);
    assert_eq!(cfg.ado_base_url.unwrap(), "https://dev.azure.com/testorg");
}

#[tokio::test]
async fn get_status_without_show_secrets_omits_config() {
    let client = Arc::new(InMemoryAzureClient::default());
    let projects = ProjectLookup::new(client);
    let params = GetStatusParams {
        show_secrets: Some(false),
        include_projects: None,
    };
    let res = projects.get_status(params).await.expect("get status ok");
    assert_eq!(res.status, "ok");
    assert_eq!(res.project_count, 1);
    assert!(res.projects.is_empty());
    assert!(res.config.is_none());
}
