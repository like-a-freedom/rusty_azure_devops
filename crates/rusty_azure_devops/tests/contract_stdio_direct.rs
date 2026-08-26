use serde_json::Value;
use serde_json::json;
use std::fs;
use std::sync::Arc;

use azure_devops_client::InMemoryAzureClient;
use rusty_azure_devops::{
    CreateWorkItemParams, GetWorkItemParams, ListProjectsParams, ProjectLookup, WorkItemOperations,
};

#[tokio::test]
async fn contract_direct_handler_outputs_match_schemas() {
    let client = Arc::new(InMemoryAzureClient::default());
    let work_items = WorkItemOperations::new(client.clone());
    let projects = ProjectLookup::new(client);

    // create_work_item
    let params = CreateWorkItemParams {
        project: "Demo".into(),
        wi_type: "Task".into(),
        fields: serde_json::from_value(json!({"System.Title":"test"})).unwrap(),
        relations: None,
        comments: None,
    };
    let res = work_items.create_work_item(params).await.unwrap();
    let out = serde_json::to_value(&res).unwrap();
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/create_work_item_result.schema.json");
    let schema_text = fs::read_to_string(schema_path).expect("read schema");
    let schema: Value = serde_json::from_str(&schema_text).expect("schema json");
    if !jsonschema::is_valid(&schema, &out) {
        panic!("schema validation failed for create_work_item result");
    }

    // get_work_item with comments
    let params2 = GetWorkItemParams {
        id: Some(rusty_azure_devops::WorkItemId::Int(123)),
        fields: None,
        expand: Some("Comments".into()),
        include_comments: None,
        include_description: None,
        relations_format: None,
    };
    let res2 = work_items.get_work_item(params2).await.unwrap();
    let out2 = serde_json::to_value(&res2).unwrap();
    let schema_path2 = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/get_work_item_result.schema.json");
    let schema_text2 = fs::read_to_string(schema_path2).expect("read schema");
    let schema2: Value = serde_json::from_str(&schema_text2).expect("schema json");
    if !jsonschema::is_valid(&schema2, &out2) {
        panic!("schema validation failed for get_work_item result");
    }

    // list_projects
    let res3 = projects
        .list_projects(ListProjectsParams {
            offset: None,
            limit: None,
        })
        .await
        .unwrap();
    let out3 = serde_json::to_value(&res3).unwrap();
    let schema_path3 = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/list_projects_result.schema.json");
    let schema_text3 = fs::read_to_string(schema_path3).expect("read schema");
    let schema3: Value = serde_json::from_str(&schema_text3).expect("schema json");
    if !jsonschema::is_valid(&schema3, &out3) {
        panic!("schema validation failed for list_projects result");
    }

    // get_status
    let res4 = projects
        .get_status(rusty_azure_devops::GetStatusParams {
            show_secrets: None,
            include_projects: None,
        })
        .await
        .unwrap();
    let out4 = serde_json::to_value(&res4).unwrap();
    let schema_path4 = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/get_status_result.schema.json");
    let schema_text4 = fs::read_to_string(schema_path4).expect("read schema");
    let schema4: Value = serde_json::from_str(&schema_text4).expect("schema json");
    if !jsonschema::is_valid(&schema4, &out4) {
        panic!("schema validation failed for get_status result");
    }

    // search_work_items
    let sparams = rusty_azure_devops::SearchWorkItemsParams {
        project: Some("Demo".into()),
        wiql: Some("SELECT [System.Id] FROM WorkItems".into()),
        details: Some(true),
        limit: Some(100),
        include_description: None,
        relations_format: None,
    };
    let sres = work_items.search_work_items(sparams).await.unwrap();
    let out5 = serde_json::to_value(&sres).unwrap();
    let schema_path5 = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/search_work_items_result.schema.json");
    let schema_text5 = fs::read_to_string(schema_path5).expect("read schema");
    let schema5: Value = serde_json::from_str(&schema_text5).expect("schema json");
    if !jsonschema::is_valid(&schema5, &out5) {
        panic!("schema validation failed for search_work_items result");
    }
}
