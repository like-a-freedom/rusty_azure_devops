use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn create_work_item_and_add_comment_flow() {
    let server = MockServer::start().await;

    // Stub create work item (include required fields expected by SDK)
    let create_resp = json!({ "id": 42, "rev": 1, "url": "http://example.local/org/proj/_workitems/42", "fields": { "System.Title": "test" } });
    Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(r".*/workitems/\$Task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(create_resp))
        .mount(&server)
        .await;

    // Stub add comment (return fields the SDK will parse)
    let comment_resp = json!({ "id": 1, "text": "hello", "createdBy": { "graphSubjectBase": { "displayName": "Tester" } }, "createdDate": "2020-01-01T00:00:00Z", "url": "http://example.local/org/proj/_apis/wit/workItems/42/comments/1" });
    Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(r".*/workItems/42/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_resp))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let patch = serde_json::json!([
        { "op": "add", "path": "/fields/System.Title", "value": "test" }
    ]);
    let wi = client
        .create_work_item("proj", "Task", &patch)
        .await
        .expect("create ok");
    assert_eq!(wi.id, 42);
    assert_eq!(wi.id, 42);

    let comment = azure_devops_client::Comment {
        id: None,
        author: None,
        text: "hello".into(),
        created_at: None,
    };
    let added = client
        .add_comment(42, &comment)
        .await
        .expect("comment added");
    assert_eq!(added.text, "hello");
}

#[tokio::test]
async fn add_comment_fallback_to_history() {
    let server = MockServer::start().await;

    // Stub comments POST to 404
    Mock::given(method("POST"))
        .and(wiremock::matchers::path_regex(
            r"^/org/proj/_apis/wit/workItems/42/comments",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_string("{}"))
        .mount(&server)
        .await;

    // Stub update work item (PATCH) to accept history patch
    let update_resp = json!({ "id": 42, "rev": 2, "url": "http://example.local/org/_workitems/42", "fields": { "System.History":"prev; hello" } });
    Mock::given(method("PATCH"))
        .and(wiremock::matchers::path_regex(r".*/workitems/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(update_resp))
        .mount(&server)
        .await;

    let client = AzureSdkClient::builder(server.uri(), "token", "org").build();
    let comment = azure_devops_client::Comment {
        id: None,
        author: None,
        text: "hello".into(),
        created_at: None,
    };
    let added = client
        .add_comment(42, &comment)
        .await
        .expect("fallback comment added");
    assert_eq!(added.text, "hello");
}
