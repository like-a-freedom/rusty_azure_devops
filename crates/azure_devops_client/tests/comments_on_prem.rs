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
use wiremock::matchers::{method, path, query_param};
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

/// Writing a comment on-prem must reach the discussion thread.
///
/// The comments API accepts only `6.0-preview`, which the SDK's api-version
/// downgrade policy rewrites to `6.0`. The server then rejects the request
/// outright ("The requested version 6.0 of the resource is under preview"),
/// and the old code silently wrote the comment to `System.History` instead.
#[tokio::test]
async fn add_comment_on_prem_posts_to_the_thread_at_the_preview_version() {
    let server = MockServer::start().await;
    mount_work_item_with_project(&server, "SIEM").await;

    Mock::given(method("POST"))
        .and(path(
            "/tfs/DefaultCollection/SIEM/_apis/wit/workItems/42/comments",
        ))
        .and(query_param("api-version", "6.0-preview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 7,
            "text": "hello",
            "createdBy": { "displayName": "A" },
            "createdDate": "2026-11-16T10:00:00Z"
        })))
        .mount(&server)
        .await;

    let comment = azure_devops_client::Comment {
        id: None,
        author: None,
        text: "hello".to_string(),
        created_at: None,
    };
    let created = client_for(&server)
        .add_comment(42, &comment)
        .await
        .expect("the comment lands in the thread");
    assert_eq!(created.id, Some(7));
    assert_eq!(created.text, "hello");

    // Not a System.History patch masquerading as a comment.
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == wiremock::http::Method::PATCH),
        "the comment must not fall back to a field patch"
    );
}

/// A missing project must surface on the write path too. The history fallback
/// cannot fix it, and it is the one case the user can act on.
#[tokio::test]
async fn add_comment_reports_a_missing_project_instead_of_writing_history() {
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

    let comment = azure_devops_client::Comment {
        id: None,
        author: None,
        text: "hello".to_string(),
        created_at: None,
    };
    let err = client_for(&server)
        .add_comment(42, &comment)
        .await
        .expect_err("a project cannot be derived");
    assert!(
        err.to_string().contains("ADO_DEFAULT_PROJECT"),
        "the actionable error must survive: {err}"
    );
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.method == wiremock::http::Method::PATCH),
        "a missing project must not be papered over with a field patch"
    );
}

/// A project name with a space must survive the trip into the URL.
#[tokio::test]
async fn a_project_name_with_a_space_is_encoded() {
    let server = MockServer::start().await;
    mount_work_item_with_project(&server, "Vulnerability Research").await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "totalCount": 0, "count": 0, "comments": []
        })))
        .mount(&server)
        .await;

    let comments = client_for(&server)
        .get_comments(42)
        .await
        .expect("the space is encoded, not dropped");
    assert!(comments.is_empty());

    let paths: Vec<String> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert!(
        paths
            .iter()
            .any(|p| p.contains("/Vulnerability%20Research/_apis/wit/workItems/42/comments")),
        "the project segment must be percent-encoded, not dropped: {paths:?}"
    );
}

/// A configured project means exactly one request, and it is the comments call.
#[tokio::test]
async fn a_configured_project_costs_exactly_one_request() {
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

    client.get_comments(42).await.expect("comments resolve");
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "a configured project must not add a lookup request"
    );
}
