use serde_json::Value;
use std::fs;

#[test]
fn schemas_are_valid_json() {
    let schema_files = vec![
        "docs/schemas/create_work_item.schema.json",
        "docs/schemas/get_work_item.schema.json",
        "docs/schemas/update_work_item.schema.json",
        "docs/schemas/search_work_items.schema.json",
        "docs/schemas/get_status.schema.json",
        "docs/schemas/list_projects.schema.json",
        // result schemas
        "docs/schemas/create_work_item_result.schema.json",
        "docs/schemas/get_work_item_result.schema.json",
        "docs/schemas/list_projects_result.schema.json",
        "docs/schemas/get_status_result.schema.json",
        "docs/schemas/search_work_items_result.schema.json",
    ];

    for f in schema_files {
        let data = fs::read_to_string(f).expect(&format!("failed to read {}", f));
        let v: Value = serde_json::from_str(&data).expect(&format!("{} is not valid json", f));
        assert!(v.get("$schema").is_some(), "{} has no $schema", f);
    }
}
