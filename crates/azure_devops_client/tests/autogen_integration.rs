use azure_devops_client::AzureSdkClient;

#[tokio::test]
async fn build_autogen_client_minimal() {
    // Create client pointing to a mock URL; this test just verifies the client builds and clones
    let c = AzureSdkClient::builder("https://example.local", "token", "org").build();
    let _c2 = c.clone();
    // no network call here; more detailed tests will stub endpoints with wiremock
}
