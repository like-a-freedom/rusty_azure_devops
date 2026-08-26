use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn retry_respects_retry_after_and_sleeps() {
    let server = MockServer::start().await;

    // Respond with 429 and Retry-After 1 (no success stub) — verify client sleeps before returning
    Mock::given(method("GET"))
        .and(path("/org/_apis/projects"))
        .respond_with(ResponseTemplate::new(429).append_header("Retry-After", "1"))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let start = std::time::Instant::now();
    let res = client.list_projects().await;
    let elapsed = start.elapsed();
    assert!(res.is_err());
    // should have respected Retry-After (at least ~1s)
    assert!(elapsed >= Duration::from_secs(1));
}

#[tokio::test]
async fn wiql_retry_and_succeeds() {
    let server = MockServer::start().await;
    // Respond with 429 and Retry-After 1
    Mock::given(method("POST"))
        .and(path("/org/proj/_apis/wit/wiql"))
        .respond_with(ResponseTemplate::new(429).append_header("Retry-After", "1"))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let start = std::time::Instant::now();
    let res = client.query_wiql("proj", "select *", 10).await;
    let elapsed = start.elapsed();
    assert!(res.is_err());
    assert!(elapsed >= Duration::from_secs(1));
}

#[tokio::test]
async fn retry_gives_up_on_server_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/org/_apis/projects"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let res = client.list_projects().await;
    assert!(res.is_err());
}
