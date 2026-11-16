//! The work item comments API requires the project segment in the URL.
//!
//! `GET .../{collection}/{project}/_apis/wit/workItems/{id}/comments` returns
//! the thread; without the project the on-prem server answers 404, even when
//! the work item has no comments. The work item itself reports its project in
//! `System.TeamProject`, so the client can derive it when no default project
//! is configured.

use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A work item that reports its project, and a comments thread under it.
async fn mount_work_item_with_project(server: &MockServer, project: &str) {
    Mock::given(method("GET"))
        .and(path("/tfs/DefaultCollection/_apis/wit/workitems/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "rev": 1,
            "url": "http://example.local/tfs/DefaultCollection/_apis/wit/workItems/42",
            "fields": {
                "System.Title": "t",
                "System.TeamProject": project
            }
        })))
        .mount(server)
        .await;
}

async fn mount_comments(server: &MockServer, project: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/tfs/DefaultCollection/{project}/_apis/wit/workItems/42/comments"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalCount": 1,
            "count": 1,
            "comments": [
                { "id": 1, "text": "hello", "createdBy": { "displayName": "A" } }
            ]
        })))
        .mount(server)
        .await;
}

fn client_for(server: &MockServer) -> AzureSdkClient {
    AzureSdkClient::builder(
        format!("{}/tfs/DefaultCollection", server.uri()),
        "token",
        "org",
    )
    .build()
    .expect("loopback HTTP mock endpoint")
}

/// The regression this file exists for: with no default project configured,
/// the comments call used to omit the project and get a 404.
#[tokio::test]
async fn comments_use_the_work_items_project_when_none_is_configured() {
    let server = MockServer::start().await;
    mount_work_item_with_project(&server, "SIEM").await;
    mount_comments(&server, "SIEM").await;

    let comments = client_for(&server)
        .get_comments(42)
        .await
        .expect("the comments thread resolves");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "hello");
    assert_eq!(comments[0].author.as_deref(), Some("A"));
}

/// A configured default project still wins, and costs no extra request.
#[tokio::test]
async fn a_configured_project_wins_and_skips_the_lookup() {
    let server = MockServer::start().await;
    mount_comments(&server, "Configured").await;

    let client = AzureSdkClient::builder(
        format!("{}/tfs/DefaultCollection", server.uri()),
        "token",
        "org",
    )
    .project("Configured")
    .build()
    .expect("loopback HTTP mock endpoint");

    let comments = client.get_comments(42).await.expect("comments resolve");
    assert_eq!(comments.len(), 1);
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().contains("/_apis/wit/workitems/")),
        "a configured project must not trigger a work item lookup"
    );
}

/// When no project is configured and the work item reports none, the failure
/// names the fix instead of surfacing a bare 404.
#[tokio::test]
async fn an_underivable_project_reports_a_config_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tfs/DefaultCollection/_apis/wit/workitems/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "rev": 1,
            "url": "http://example.local/tfs/DefaultCollection/_apis/wit/workItems/42",
            "fields": { "System.Title": "t" }
        })))
        .mount(&server)
        .await;

    let err = client_for(&server)
        .get_comments(42)
        .await
        .expect_err("a project cannot be derived");
    let text = err.to_string();
    assert!(
        text.contains("ADO_DEFAULT_PROJECT"),
        "the error must name the fix: {text}"
    );
}
