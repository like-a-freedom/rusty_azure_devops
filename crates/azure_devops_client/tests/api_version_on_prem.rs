//! On-prem, no configured `ado_api_version`: SDK calls must still be
//! downgraded to a version the server accepts (ADR-0005).
//!
//! The regression is silent in every other test because they either configure
//! `api_version("6.0")` explicitly or use a mock that answers 200 whatever the
//! query says. The real server answers 400 `VssVersionOutOfRangeException`
//! for `7.1-preview`, `7.1` and `7.0`, and 200 only for `6.0`.

use azure_devops_client::AzureDevOpsClient;
use azure_devops_client::AzureSdkClient;
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The api-version an SDK-generated request actually carries on an on-prem
/// endpoint when nothing was configured.
async fn api_version_sent_without_configuration(operation: &str) -> String {
    let server = MockServer::start().await;

    for verb in ["POST", "PUT", "PATCH"] {
        Mock::given(method(verb))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 9349327,
                "rev": 2,
                "url": "http://example.local/tfs/DefaultCollection/_apis/wit/workItems/9349327",
                "fields": { "System.Title": "test" }
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 9349327,
            "rev": 1,
            "url": "http://example.local/tfs/DefaultCollection/_apis/wit/workItems/9349327",
            "fields": { "System.Title": "test" }
        })))
        .mount(&server)
        .await;

    let endpoint = format!("{}/tfs/DefaultCollection/", server.uri());
    let client = AzureSdkClient::builder(endpoint, "token", "org")
        .build()
        .expect("loopback HTTP mock endpoint");

    match operation {
        "create" => client
            .create_work_item(
                "SomeProject",
                "Task",
                &json!([
                    { "op": "add", "path": "/fields/System.Title", "value": "t" }
                ]),
            )
            .await
            .expect("create must work on-prem with no configured api version"),
        "update" => client
            .update_work_item(
                9349327,
                &json!([
                    { "op": "replace", "path": "/fields/System.State", "value": "Active" }
                ]),
                None,
            )
            .await
            .expect("update must work on-prem with no configured api version"),
        other => unreachable!("unknown operation: {other}"),
    };

    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests.is_empty(),
        "no request was sent for the {operation} operation"
    );
    let version = requests[0]
        .url
        .query_pairs()
        .find(|(key, _)| key == "api-version")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "<none>".to_string());
    assert_eq!(
        version, "6.0",
        "the {operation} operation sent api-version={version}; on-prem servers \
         accept at most 6.0, so a fresh config that does not set ado_api_version \
         turns every SDK-only call into a 400 VssVersionOutOfRangeException"
    );
    version
}

#[tokio::test]
async fn create_downgrades_to_6_0_without_a_configured_api_version() {
    let version = api_version_sent_without_configuration("create").await;
    assert_eq!(version, "6.0");
}

#[tokio::test]
async fn update_downgrades_to_6_0_without_a_configured_api_version() {
    let version = api_version_sent_without_configuration("update").await;
    assert_eq!(version, "6.0");
}
