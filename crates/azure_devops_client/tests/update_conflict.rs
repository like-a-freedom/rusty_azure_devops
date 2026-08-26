use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn update_conflict_returns_error() {
    let server = MockServer::start().await;

    // Stub update to return 409 (SDK will include org/project in path)
    Mock::given(method("PATCH"))
        .and(path("/org/_apis/wit/workitems/42"))
        .respond_with(ResponseTemplate::new(409).set_body_string("{}"))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let patch = serde_json::json!([{ "op":"add", "path":"/fields/System.Title", "value":"x" }]);
    let res = client.update_work_item(42, &patch, Some(1)).await;
    assert!(res.is_err());
}
