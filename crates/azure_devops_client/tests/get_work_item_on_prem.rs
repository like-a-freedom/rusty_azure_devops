use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_work_item_on_prem() {
    // Start a mock server and simulate an on-prem ADO base URL that includes a collection
    let server = MockServer::start().await;

    // Respond with a valid work item JSON for any GET; we'll inspect the actual received request path below
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 9349327,
            "rev": 1,
            "url": "http://example.local/tfs/DefaultCollection/_apis/wit/workItems/9349327",
            "fields": { "System.Title": "test" }
        })))
        .mount(&server)
        .await;

    // Create client with an endpoint that includes trailing slash and collection segment
    let endpoint = format!("{}/tfs/DefaultCollection/", server.uri());
    let client = AzureSdkClient::builder(endpoint, "token", "org").build();

    // Perform get_work_item which should hit the server
    let wi = client
        .get_work_item(9349327, None, None)
        .await
        .expect("get works");
    assert_eq!(wi.id, 9349327);

    // Inspect what path the server actually received and ensure there are no duplicate slashes before _apis
    let requests = server.received_requests().await.unwrap();
    assert!(!requests.is_empty(), "no requests received by mock server");
    let path = &requests[0].url.path();
    assert!(
        !path.contains("///_apis"),
        "found triple slashes in path: {}",
        path
    );
    assert!(
        path.contains("/tfs/DefaultCollection/_apis/wit/workitems/9349327"),
        "unexpected path: {}",
        path
    );
}
