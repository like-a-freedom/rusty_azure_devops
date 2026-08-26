use azure_devops_client::InMemoryAzureClient;
use rusty_azure_devops::{GetWorkItemParams, WorkItemOperations};
use std::sync::Arc;

#[tokio::test]
async fn missing_id_returns_helpful_error() {
    let client = Arc::new(InMemoryAzureClient::default());
    let work_items = WorkItemOperations::new(client);
    let params = GetWorkItemParams {
        id: None,
        fields: None,
        expand: None,
        include_comments: None,
        include_description: None,
        relations_format: None,
    };
    let res = work_items.get_work_item(params).await;
    assert!(res.is_err(), "expected error when id missing");
    let err = res.err().unwrap();
    assert!(err.to_string().contains("missing required parameter: id"));
}
