use std::env;

// E2E playback test. Skipped unless `TEST_AZURE_RECORDING` is set.
//
// Loads recorded HTTP fixtures from `tests/fixtures/sample_playback*.json`,
// spins up a wiremock server that replays them, and verifies the SDK client
// can consume the replayed responses.
//
// Note: this test was rewritten to use `AzureSdkClient::builder` because
// `AzureSdkClient::new` no longer exists (the builder is the only constructor
// after ADR-0005 / ADR-0007).
#[tokio::test]
async fn e2e_playback_fixture() {
    if env::var("TEST_AZURE_RECORDING").is_err() {
        eprintln!(
            "Skipping e2e playback test: set TEST_AZURE_RECORDING=1 to run (requires a real Azure DevOps endpoint for recording)"
        );
        return;
    }

    let mut fixtures_files = Vec::new();
    for name in [
        "tests/fixtures/playback.json",
        "tests/fixtures/sample_playback.json",
        "tests/fixtures/sample_playback_create.json",
    ] {
        let p = std::path::Path::new(name);
        if p.exists() {
            fixtures_files.push(p.to_path_buf());
        }
    }
    assert!(
        !fixtures_files.is_empty(),
        "no playback fixtures found under tests/fixtures/"
    );

    for path in fixtures_files.iter() {
        let data = std::fs::read_to_string(path).expect("fixture file");
        let fixtures: serde_json::Value = serde_json::from_str(&data).expect("valid json");

        let server = wiremock::MockServer::start().await;
        if let Some(arr) = fixtures.as_array() {
            for item in arr.iter() {
                let method = item
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("GET");
                let path = item.get("path").and_then(|p| p.as_str()).unwrap_or("/");
                let status = item
                    .get("status")
                    .and_then(|s| s.as_u64())
                    .unwrap_or(200) as u16;
                let body = item
                    .get("body")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let resp = wiremock::ResponseTemplate::new(status).set_body_json(body);
                let matcher = match method {
                    "POST" => wiremock::Mock::given(wiremock::matchers::method("POST"))
                        .and(wiremock::matchers::path(path))
                        .respond_with(resp),
                    _ => wiremock::Mock::given(wiremock::matchers::method("GET"))
                        .and(wiremock::matchers::path(path))
                        .respond_with(resp),
                };
                matcher.mount(&server).await;
            }
        }

        // Use the builder (the only constructor after the resilience split).
        // Production code would supply a real RetryStrategy + OnPremFallback; for
        // playback, the NoOp defaults are fine.
        let client = azure_devops_client::AzureSdkClient::builder(&server.uri(), "token", "")
            .build();
        let p = client.list_projects().await.expect("projects");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "Demo");
    }
}
