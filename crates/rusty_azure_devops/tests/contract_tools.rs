use azure_devops_client::AzureDevOpsClient;
use rusty_azure_devops::CreateWorkItemParams;
use serde_json::Value;
use std::fs;

#[tokio::test]
async fn tools_output_matches_schemas() {
    let client = azure_devops_client::InMemoryAzureClient::default();

    // Validate that the sample input for `create_work_item` conforms to the input schema
    let params = CreateWorkItemParams {
        project: "Demo".into(),
        wi_type: "Task".into(),
        fields: serde_json::from_value(serde_json::json!({"System.Title":"test"})).unwrap(),
        relations: None,
        comments: None,
    };
    let input_json = serde_json::to_value(&params).unwrap();
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/create_work_item.schema.json");
    let schema_text = fs::read_to_string(schema_path).expect("schema file");
    let schema: Value = serde_json::from_str(&schema_text).expect("schema json");
    if !jsonschema::is_valid(&schema, &input_json) {
        panic!("schema validation failed for create_work_item input");
    }

    // Validate that sample input for list_projects conforms to schema (empty input allowed)
    let input2 = serde_json::json!({});
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/list_projects.schema.json");
    let schema_text = fs::read_to_string(schema_path).expect("schema file");
    let schema: Value = serde_json::from_str(&schema_text).expect("schema json");
    if !jsonschema::is_valid(&schema, &input2) {
        panic!("schema validation failed for list_projects input");
    }

    // Do a quick sanity check of produced outputs
    let wi: azure_devops_client::WorkItem = client
        .create_work_item("Demo", "Task", &serde_json::json!([]))
        .await
        .unwrap();
    assert!(wi.id != 0);
    let projects: Vec<azure_devops_client::Project> = client.list_projects().await.unwrap();
    assert!(!projects.is_empty());
}
