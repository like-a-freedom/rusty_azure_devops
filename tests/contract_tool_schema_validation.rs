use serde_json::json;
use std::fs;

#[test]
fn create_work_item_example_matches_schema() {
    let schema =
        fs::read_to_string("docs/schemas/create_work_item.schema.json").expect("read schema");
    let schema_json: serde_json::Value = serde_json::from_str(&schema).expect("parse schema");

    let example = json!({
        "project": "Demo",
        "type": "Task",
        "fields": { "System.Title": "Example" },
        "comments": [{ "author": "alice", "text": "note" }]
    });

    assert!(
        jsonschema::is_valid(&schema_json, &example),
        "example should validate against schema"
    );
}
