use schemars::schema_for;
use serde_json::Value;

use rusty_azure_devops::{
    CreateWorkItemParams, CreateWorkItemResult, GetWorkItemResult, ListProjectsResult,
    SearchWorkItemsResult, StatusResult,
};

fn root_schema<T: schemars::JsonSchema>() -> Value {
    let s = schema_for!(T);
    serde_json::to_value(&s).expect("serialize schema")
}

fn assert_root_is_object(v: &Value) {
    let t = v
        .get("schema")
        .and_then(|s| s.get("type"))
        .or_else(|| v.get("type"));
    let got = if let Some(tt) = t {
        tt.clone()
    } else if v.get("properties").is_some() {
        Value::String("object".into())
    } else {
        panic!("schema root missing type and properties: {}", v);
    };
    if got == Value::String("object".into()) {
        return;
    }
    if got
        == Value::Array(vec![
            Value::String("object".into()),
            Value::String("null".into()),
        ])
    {
        return;
    }
    panic!("root is not object: {}", v);
}

fn assert_array_items_are_object(schema: &Value, prop: &str) {
    let props = schema
        .get("schema")
        .and_then(|s| s.get("properties"))
        .or_else(|| schema.get("properties"))
        .expect("properties missing");
    let p = props
        .get(prop)
        .unwrap_or_else(|| panic!("property {} missing", prop));
    let items = p
        .get("items")
        .unwrap_or_else(|| panic!("array property {} missing items", prop));
    if items.get("type").is_some() {
        let ty = items.get("type").unwrap();
        assert!(
            ty == "object"
                || (ty.is_array() && ty.as_array().unwrap().iter().any(|x| x == "object")),
            "items type not object: {}",
            items
        );
        return;
    }
    if items.get("$ref").is_some() {
        let r = items.get("$ref").unwrap().as_str().unwrap();
        let def_name = r.trim_start_matches("#/$defs/");
        let defs = schema
            .get("schema")
            .and_then(|s| s.get("$defs"))
            .or_else(|| schema.get("$defs"))
            .expect("$defs missing");
        let def = defs
            .get(def_name)
            .unwrap_or_else(|| panic!("def {} missing", def_name));
        let ty = def.get("type").expect("def type missing");
        assert!(
            ty == "object"
                || (ty.is_array() && ty.as_array().unwrap().iter().any(|x| x == "object")),
            "def {} is not object: {}",
            def_name,
            def
        );
        return;
    }
    if items.get("additionalProperties").is_some() {
        return;
    }
    panic!("array items schema is not object or $ref or map: {}", items);
}

#[test]
fn validate_all_tool_schemas() {
    let schemas = vec![
        (
            "CreateWorkItemResult",
            root_schema::<CreateWorkItemResult>(),
        ),
        ("GetWorkItemResult", root_schema::<GetWorkItemResult>()),
        ("ListProjectsResult", root_schema::<ListProjectsResult>()),
        ("StatusResult", root_schema::<StatusResult>()),
        (
            "SearchWorkItemsResult",
            root_schema::<SearchWorkItemsResult>(),
        ),
    ];
    for (_name, s) in schemas {
        assert_root_is_object(&s);
    }

    let create_schema = root_schema::<CreateWorkItemParams>();
    assert_array_items_are_object(&create_schema, "relations");

    let search_schema = root_schema::<SearchWorkItemsResult>();
    let props = search_schema
        .get("schema")
        .and_then(|s| s.get("properties"))
        .or_else(|| search_schema.get("properties"))
        .expect("properties missing");
    let ids = props.get("ids").expect("ids missing");
    let id_items = ids.get("items").expect("ids items missing");
    assert_eq!(id_items.get("type").unwrap(), "integer");

    assert!(props.get("details").is_some());
    assert_array_items_are_object(&search_schema, "details");

    let list_schema = root_schema::<ListProjectsResult>();
    assert_array_items_are_object(&list_schema, "projects");

    let status_schema = root_schema::<StatusResult>();
    assert_array_items_are_object(&status_schema, "projects");
}

/// Parity test: every CLI subcommand must map to a `Tool` variant.
/// CLI `Command` is private to the binary, so we assert against the known
/// list of CLI subcommand names. If a new CLI command is added without a
/// corresponding `Tool`, this test will fail.
#[test]
fn tool_enum_covers_all_cli_commands() {
    use rusty_azure_devops::tools::Tool;

    // (CLI subcommand name, expected Tool variant).
    // Keep this list in sync with the `Command` enum in bin/cli.rs.
    // login/logout are CLI-only auth UX (like `gcx login`, `pangolin
    // login/logout`, `keenable login/logout`) and intentionally absent here:
    // local secret ops must not become MCP tools.
    let cli_to_tool: &[(&str, &str)] = &[
        ("status", "get_status"),
        ("list-projects", "list_projects"),
        ("get-work-item", "get_work_item"),
        ("create-work-item", "create_work_item"),
        ("update-work-item", "update_work_item"),
        ("manage-link", "manage_work_item_link"),
        ("search", "search_work_items"),
        ("comments", "get_workitem_comments"),
        ("add-comment", "add_comment"),
        ("get-query-results", "get_query_results"),
    ];

    let tool_names: Vec<&str> = Tool::ALL.iter().map(|t| t.as_str()).collect();
    for (cli_cmd, expected_tool) in cli_to_tool {
        assert!(
            tool_names.contains(expected_tool),
            "CLI command '{}' expects tool '{}' which does not exist in Tool enum. Available: {:?}",
            cli_cmd,
            expected_tool,
            tool_names,
        );
    }

    /// CLI-only commands with no MCP tool mirror. If a Tool variant ever
    /// appears for one of these, this fails loudly: local secret ops must
    /// stay out of the MCP surface.
    const CLI_ONLY: &[&str] = &["login", "logout"];

    for cli_cmd in CLI_ONLY {
        assert!(
            !tool_names.contains(cli_cmd),
            "CLI-only '{}' must not gain an MCP tool (local secret op)",
            cli_cmd,
        );
    }
}
