use rmcp::Json;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::domain::*;
use crate::operations::{
    CommentOperations, LinkOperations, ProjectLookup, QueryOperations, WorkItemOperations,
};

/// Single source of truth for MCP tool names. The parity test in
/// `tests/schema_validation.rs` asserts that every CLI command maps to a
/// tool here, but not every tool must have a CLI mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    GetStatus,
    ListProjects,
    GetWorkItem,
    CreateWorkItem,
    UpdateWorkItem,
    ManageWorkItemLink,
    SearchWorkItems,
    GetWorkitemComments,
    GetQueryResults,
    AddComment,
}

impl Tool {
    /// All tool names in display order.
    pub const ALL: &'static [Tool] = &[
        Tool::GetStatus,
        Tool::ListProjects,
        Tool::GetWorkItem,
        Tool::CreateWorkItem,
        Tool::UpdateWorkItem,
        Tool::ManageWorkItemLink,
        Tool::SearchWorkItems,
        Tool::GetWorkitemComments,
        Tool::GetQueryResults,
        Tool::AddComment,
    ];

    /// Tool names as a comma-separated string for the instructions blurb.
    pub fn all_names() -> String {
        Self::ALL
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The MCP tool name string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Tool::GetStatus => "get_status",
            Tool::ListProjects => "list_projects",
            Tool::GetWorkItem => "get_work_item",
            Tool::CreateWorkItem => "create_work_item",
            Tool::UpdateWorkItem => "update_work_item",
            Tool::ManageWorkItemLink => "manage_work_item_link",
            Tool::SearchWorkItems => "search_work_items",
            Tool::GetWorkitemComments => "get_workitem_comments",
            Tool::GetQueryResults => "get_query_results",
            Tool::AddComment => "add_comment",
        }
    }
}

#[derive(Clone)]
pub struct AdoMcpHandler {
    work_items: WorkItemOperations,
    links: LinkOperations,
    comments: CommentOperations,
    projects: ProjectLookup,
    queries: QueryOperations,
    #[allow(dead_code)]
    tool_router: rmcp::handler::server::tool::ToolRouter<AdoMcpHandler>,
}

impl AdoMcpHandler {
    pub fn new(
        work_items: WorkItemOperations,
        links: LinkOperations,
        comments: CommentOperations,
        projects: ProjectLookup,
        queries: QueryOperations,
    ) -> Self {
        Self {
            work_items,
            links,
            comments,
            projects,
            queries,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl AdoMcpHandler {
    #[tool(
        name = "get_status",
        description = "Get server status — connection health, project count, and configured default project. Set show_secrets=true to see token_present, base_url, and default_project (these are not secrets). Set include_projects=true to list all project names."
    )]
    pub(crate) async fn get_status(
        &self,
        params: Parameters<GetStatusParams>,
    ) -> Result<Json<StatusResult>, String> {
        Ok(Json(
            self.projects
                .get_status(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "list_projects",
        description = "List Azure DevOps team projects. Supports pagination via offset/limit. Default returns all projects. Use offset and limit to page through large lists."
    )]
    pub(crate) async fn list_projects(
        &self,
        params: Parameters<ListProjectsParams>,
    ) -> Result<Json<ListProjectsResult>, String> {
        Ok(Json(
            self.projects
                .list_projects(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "create_work_item",
        description = "Create a work item in a project. Requires 'fields' with 'System.Title'. Supports arbitrary field reference names — just add them as keys in the 'fields' object (e.g. 'Microsoft.VSTS.CMMI.RequirementType', 'System.Tags', 'Custom.*'). Also supports 'relations' (array of {url, rel, attributes?}) and 'comments' (array of {text, author?, created_at?}), both optional.\n\nSystem.Description expects HTML (Azure's native format). The CLI auto-converts Markdown to HTML for --description and --field, but this MCP tool passes fields as-is.\n\nCommon field examples:\n  {\"fields\": {\"System.Title\": \"My WI\", \"System.Tags\": \"urgent\", \"Microsoft.VSTS.CMMI.RequirementType\": \"Functional\"}}\n\nUse get_work_item after creation to inspect the result."
    )]
    pub(crate) async fn create_work_item(
        &self,
        params: Parameters<CreateWorkItemParams>,
    ) -> Result<Json<CreateWorkItemResult>, String> {
        Ok(Json(
            self.work_items
                .create_work_item(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "update_work_item",
        description = "Update a work item using a JSON Patch document. Optional expected_rev enables optimistic concurrency (prevents overwriting concurrent edits). Use to modify fields (add/replace), relations, or tags.\n\nExample — set field value:\n  {\"id\": 123, \"patch\": [{\"op\":\"add\",\"path\":\"/fields/System.Tags\",\"value\":\"urgent\"}]}\n\nExample — add relation:\n  {\"id\": 123, \"patch\": [{\"op\":\"add\",\"path\":\"/relations/-\",\"value\":{\"rel\":\"System.LinkTypes.Related\",\"url\":\"https://.../workItems/456\"}}]}\n\nTip: use 'replace' op for existing fields, 'add' for new fields. Remove fields with 'remove' op."
    )]
    pub(crate) async fn update_work_item(
        &self,
        params: Parameters<UpdateWorkItemParams>,
    ) -> Result<Json<GetWorkItemResult>, String> {
        Ok(Json(
            self.work_items
                .update_work_item(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "manage_work_item_link",
        description = "Add or remove a relation between work items. Actions: 'add' (create link) or 'remove' (delete link). source_id and target are work item IDs (integers). For Hyperlinks/Remote links, pass a URL as target. Optional link_type (default: 'Related') and attributes.comment for relation comment.\n\nCommon link types: System.LinkTypes.Related, Hierarchy-Forward, Dependency-Forward, Microsoft.VSTS.Common.Implementation, Hyperlink.\n\nTip: use create_work_item with 'relations' to link during creation."
    )]
    pub(crate) async fn manage_work_item_link(
        &self,
        params: Parameters<ManageWorkItemLinkParams>,
    ) -> Result<Json<ManageWorkItemLinkResult>, String> {
        Ok(Json(
            self.links
                .manage_work_item_link(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "search_work_items",
        description = "Search work items using WIQL or simple filters. Returns IDs by default; set details=true to return full records with token-efficient fields (id, type, state, title, assignee).\n\nParameters:\n- wiql: WIQL query string\n- project: project name (default: first available)\n- details: true to return full records (default: false)\n- limit: max results (default: 100)\n- include_description: true to include descriptions in details (HTML→Markdown) (default: false)\n- relations_format: 'ids' (default) or 'urls' for relation URLs"
    )]
    pub(crate) async fn search_work_items(
        &self,
        params: Parameters<SearchWorkItemsParams>,
    ) -> Result<Json<SearchWorkItemsResult>, String> {
        Ok(Json(
            self.work_items
                .search_work_items(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "get_work_item",
        description = "Retrieve a work item by id. Default fields include id, rev, type, state, title, assignee, dates, tags, and team project. Pass specific fields to get only what you need.\n\nParameters:\n- fields: list of field names to return (default: id/rev/type/state/title/assignee/createdDate/changedDate/tags/teamProject). Use [\"*\"] for ALL fields.\n- include_comments: true to fetch discussion thread (default: false)\n- include_description: true to fetch System.Description as Markdown (HTML converted to Markdown) (default: false)\n- relations_format: 'ids' (default, returns work item IDs) or 'urls' (full API URLs)\n- $expand: 'relations', 'Comments', or 'All' for Azure API expand\n\nOn-prem TFS: $expand drops fields parameter (server limitation). Use\ninclude_comments=true instead of $expand='Comments' (not supported on-prem).\n\nDescription round-trip: get returns Markdown; when writing back via create_work_item\nor update_work_item fields, Markdown is auto-converted to HTML (Azure's native format).\n\nUse search_work_items first to find work items, then get_work_item to inspect details."
    )]
    pub(crate) async fn get_work_item(
        &self,
        params: Parameters<GetWorkItemParams>,
    ) -> Result<Json<GetWorkItemResult>, String> {
        Ok(Json(
            self.work_items
                .get_work_item(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "get_workitem_comments",
        description = "Retrieve all comments for a work item by id. Returns comment text, author, and creation date. Use to review discussion history. Recommended on-prem: use this tool instead of get_work_item with $expand='Comments' (not supported on-prem)."
    )]
    pub(crate) async fn get_workitem_comments(
        &self,
        params: Parameters<GetWorkitemCommentsParams>,
    ) -> Result<Json<GetWorkitemCommentsResult>, String> {
        Ok(Json(
            self.comments
                .get_workitem_comments(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "get_query_results",
        description = "Execute a saved Azure DevOps query by URL or (project, query_id). Returns work item IDs that can be fetched with get_work_item. Supports query URLs from both on-prem (tfs/Collection/Project/_queries/query/GUID) and Azure DevOps cloud.\n\nParameters:\n- url: full query URL (e.g. https://host/Project/_queries/query/GUID)\n- project: project name (required when using query_id)\n- query_id: query GUID (requires project)\n- offset: number of IDs to skip (default: 0). Use with limit to paginate through large result sets\n- limit: max results to return (default: 200)\n\nResponse includes `total` reflecting the full query result count regardless of offset/limit — if total > ids.length(), there are more results beyond the current page.\n\nExamples:\n- By URL: { \"url\": \"https://tfs.example.com/tfs/DefaultCollection/MyProject/_queries/query/1c3ed7b3-17bb-4fe9-9b64-256f118ebebf\" }\n- By params: { \"project\": \"MyProject\", \"query_id\": \"1c3ed7b3-17bb-4fe9-9b64-256f118ebebf\" }\n- Paginate: { \"url\": \"...\", \"offset\": 200, \"limit\": 100 }"
    )]
    pub(crate) async fn get_query_results(
        &self,
        params: Parameters<GetQueryParams>,
    ) -> Result<Json<GetQueryResult>, String> {
        Ok(Json(
            self.queries
                .get_query(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }

    #[tool(
        name = "add_comment",
        description = "Add a discussion comment to a work item by ID. The comment appears in the work item's discussion thread. On-prem TFS: falls back to System.History field if the Comments API is unavailable."
    )]
    pub(crate) async fn add_comment(
        &self,
        params: Parameters<AddCommentParams>,
    ) -> Result<Json<AddCommentResult>, String> {
        Ok(Json(
            self.comments
                .add_comment(params.0)
                .await
                .map_err(|e| e.to_string())?,
        ))
    }
}

use rmcp::tool_handler;

#[tool_handler]
impl rmcp::ServerHandler for AdoMcpHandler {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        let capabilities = rmcp::model::ServerCapabilities::builder()
            .enable_tools()
            .build();

        rmcp::model::ServerConfig::new(capabilities).with_instructions(format!(
            "Azure DevOps MCP server — manage work items, projects, queries, and comments. Tools: {}. Start with get_status to verify connectivity and find your default project.",
            Tool::all_names(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azure_devops_client::InMemoryAzureClient;
    use std::sync::Arc;

    #[test]
    fn handler_get_info() {
        let h = test_handler();
        let info = rmcp::ServerHandler::get_info(&h);
        assert!(info.instructions.is_some());
        assert!(info.instructions.unwrap().contains("Azure DevOps"));
    }

    fn test_handler() -> AdoMcpHandler {
        let base = InMemoryAzureClient::default();
        // Seed work items that several link-management tests query by id.
        {
            let mut work_items = base.work_items.lock().unwrap();
            for id in 100..=300 {
                let next = id + 1;
                work_items
                    .entry(id)
                    .or_insert_with(|| azure_devops_client::WorkItem {
                        id,
                        rev: Some(1),
                        fields: serde_json::json!({"System.Title": "seed"}),
                        comments: None,
                        relations: Some(vec![azure_devops_client::Relation {
                            rel: Some("System.LinkTypes.Related".to_string()),
                            url: format!("https://example.com/workItems/{}", next),
                            attributes: None,
                        }]),
                    });
            }
        }
        let client = Arc::new(base);
        AdoMcpHandler::new(
            WorkItemOperations::new(client.clone()),
            LinkOperations::new(client.clone()),
            CommentOperations::new(client.clone()),
            ProjectLookup::new(client.clone()),
            QueryOperations::new(client),
        )
    }

    #[tokio::test]
    async fn handler_basic_get_status() {
        let h = test_handler();
        let res = h
            .get_status(Parameters(GetStatusParams {
                show_secrets: None,
                include_projects: None,
            }))
            .await
            .unwrap();
        assert_eq!(res.0.status, "ok");
        assert_eq!(res.0.project_count, 1);
        assert!(res.0.projects.is_empty());
    }

    #[tokio::test]
    async fn handler_create_work_item() {
        let h = test_handler();
        let params = CreateWorkItemParams {
            project: "Demo".into(),
            wi_type: "Task".into(),
            fields: serde_json::from_value(serde_json::json!({"System.Title":"Test from handler"}))
                .unwrap(),
            relations: None,
            comments: None,
        };
        let res = h.create_work_item(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 123);
    }

    #[tokio::test]
    async fn handler_get_work_item_with_comments() {
        let h = test_handler();
        let params = GetWorkItemParams {
            id: Some(WorkItemId::Int(123)),
            fields: None,
            expand: Some("Comments".into()),
            include_comments: None,
            include_description: None,
            relations_format: None,
        };
        let res = h.get_work_item(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 123);
        assert!(res.0.comments.is_some());
        let comments = res.0.comments.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "hello");
    }

    #[tokio::test]
    async fn handler_get_work_item_with_include_comments_flag() {
        let h = test_handler();
        let params = GetWorkItemParams {
            id: Some(WorkItemId::Int(123)),
            fields: None,
            expand: None,
            include_comments: Some(true),
            include_description: None,
            relations_format: None,
        };
        let res = h.get_work_item(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 123);
        assert!(res.0.comments.is_some());
        let comments = res.0.comments.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "hello");
    }

    #[test]
    fn deser_get_work_item_params_accepts_int_and_string() {
        let json_int = r#"{ "id": 123 }"#;
        let p_int: GetWorkItemParams = serde_json::from_str(json_int).expect("deserialize int");
        match p_int.id {
            Some(WorkItemId::Int(i)) => assert_eq!(i, 123),
            _ => panic!("expected int variant"),
        }

        let json_str = r#"{ "id": "123" }"#;
        let p_str: GetWorkItemParams = serde_json::from_str(json_str).expect("deserialize str");
        match p_str.id {
            Some(WorkItemId::Str(s)) => assert_eq!(s, "123"),
            _ => panic!("expected str variant"),
        }

        let json_missing = r#"{ }"#;
        let res: Result<GetWorkItemParams, _> = serde_json::from_str(json_missing);
        assert!(res.is_ok());
        let p_missing = res.expect("parse missing");
        assert!(p_missing.id.is_none());
    }

    #[tokio::test]
    async fn handler_get_work_item_with_string_id() {
        let h = test_handler();
        let params = GetWorkItemParams {
            id: Some(WorkItemId::Str("9349327".into())),
            fields: None,
            expand: None,
            include_comments: None,
            include_description: None,
            relations_format: None,
        };
        let res = h.get_work_item(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 9349327);
    }

    #[tokio::test]
    async fn handler_get_workitem_comments() {
        let h = test_handler();
        let params = GetWorkitemCommentsParams {
            id: WorkItemId::Int(123),
        };
        let res = h.get_workitem_comments(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 123);
        assert_eq!(res.0.total, 1);
        assert_eq!(res.0.comments[0].text, "hello");
    }

    #[tokio::test]
    async fn manage_work_item_link_add_removes() {
        let h = test_handler();

        let params = ManageWorkItemLinkParams {
            action: "add".to_string(),
            source_id: WorkItemId::Int(100),
            target: WorkItemId::Int(101),
            link_type: Some("System.LinkTypes.Related".to_string()),
            attributes: None,
        };
        let res = h.manage_work_item_link(Parameters(params)).await.unwrap();
        assert!(res.0.success);
        assert!(res.0.relations.is_some());
        let rels = res.0.relations.unwrap();
        assert!(rels.iter().any(|r| r.url.ends_with("/101")));

        let params2 = ManageWorkItemLinkParams {
            action: "remove".to_string(),
            source_id: WorkItemId::Int(100),
            target: WorkItemId::Int(101),
            link_type: None,
            attributes: None,
        };
        let res2 = h.manage_work_item_link(Parameters(params2)).await.unwrap();
        assert!(res2.0.success);
    }

    #[tokio::test]
    async fn update_work_item_azure_missing_fields_error_is_wrapped() {
        use crate::test_helpers::missing_fields_client;
        let bad = missing_fields_client();
        let h = AdoMcpHandler::new(
            WorkItemOperations::new(bad.clone()),
            LinkOperations::new(bad.clone()),
            CommentOperations::new(bad.clone()),
            ProjectLookup::new(bad.clone()),
            QueryOperations::new(bad),
        );
        let params = UpdateWorkItemParams {
            id: WorkItemId::Int(999),
            patch: Some(serde_json::json!([
                {"op":"add","path":"/relations/-","value":{"rel":"System.LinkTypes.Related","url":"https://example.com/workItems/1"}}
            ])),
            tags: None,
            expected_rev: None,
        };
        let res = h.update_work_item(Parameters(params)).await;
        assert!(res.is_err());
        let err = res.err().unwrap().to_string();
        assert!(err.contains("missing field `fields`"));
        assert!(err.contains("To add a relation to an existing work item"));
    }

    #[tokio::test]
    async fn handler_add_comment() {
        let h = test_handler();
        let params = AddCommentParams {
            id: WorkItemId::Int(123),
            text: "handler test comment".into(),
        };
        let res = h.add_comment(Parameters(params)).await.unwrap();
        assert_eq!(res.0.id, 123);
        assert_eq!(res.0.text, "handler test comment");
    }

    #[tokio::test]
    async fn manage_work_item_link_various_types() {
        let h = test_handler();

        let rel_types = [
            "System.LinkTypes.Related",
            "System.LinkTypes.Hierarchy-Forward",
            "System.LinkTypes.Hierarchy-Reverse",
            "System.LinkTypes.Dependency-Forward",
            "System.LinkTypes.Dependency-Reverse",
            "Microsoft.VSTS.Common.Implementation",
        ];

        for rel in rel_types.iter() {
            let params = ManageWorkItemLinkParams {
                action: "add".to_string(),
                source_id: WorkItemId::Int(200),
                target: WorkItemId::Int(201),
                link_type: Some(rel.to_string()),
                attributes: None,
            };
            let res = h.manage_work_item_link(Parameters(params)).await.unwrap();
            assert!(res.0.success, "add failed for rel {}", rel);
            let rels = res.0.relations.unwrap_or_default();
            assert!(
                rels.iter()
                    .any(|r| r.rel == *rel && r.url.ends_with("/201")),
                "relation with rel {} not found",
                rel
            );
        }

        let params2 = ManageWorkItemLinkParams {
            action: "remove".to_string(),
            source_id: WorkItemId::Int(200),
            target: WorkItemId::Int(201),
            link_type: None,
            attributes: None,
        };
        let res2 = h.manage_work_item_link(Parameters(params2)).await.unwrap();
        assert!(res2.0.success);
    }

    #[test]
    fn manage_work_item_link_deserialize_hints_on_misnamed_keys() {
        let bad = serde_json::json!({
            "action": "add",
            "relationType": "KL.CommonImplementationInfo-Forward",
            "sourceWorkItemId": 100,
            "targetWorkItemId": 200
        });

        let res: Result<ManageWorkItemLinkParams, _> = serde_json::from_value(bad);
        assert!(res.is_err());
        let err = res.err().unwrap().to_string();
        assert!(err.contains("missing required fields"));
        assert!(err.contains("use `link_type`"));
        assert!(err.contains("use `source_id`"));
    }

    #[test]
    fn manage_work_item_link_detects_relationtype_with_valid_keys() {
        let bad = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "relationType": "KL.CommonImplementationInfo-Forward",
            "comment": "Linked from SIEM roadmap CSV import (automation)"
        });

        let res: Result<ManageWorkItemLinkParams, _> = serde_json::from_value(bad);
        assert!(res.is_err());
        let err = res.err().unwrap().to_string();
        assert!(err.contains("use `link_type`"));
    }

    #[test]
    fn manage_work_item_link_accepts_comment_shorthand() {
        let payload = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "link_type": "KL.CommonImplementationInfo-Forward",
            "comment": "Linked from SIEM roadmap CSV import"
        });
        let res: ManageWorkItemLinkParams = serde_json::from_value(payload).unwrap();
        assert!(res.attributes.is_some());
        let attrs = res.attributes.unwrap();
        assert!(attrs.get("comment").is_some());
        assert_eq!(
            attrs.get("comment").unwrap(),
            &serde_json::json!("Linked from SIEM roadmap CSV import")
        );
    }

    #[test]
    fn manage_work_item_link_rejects_fields_key() {
        let payload = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "link_type": "KL.CommonImplementationInfo-Forward",
            "fields": {}
        });
        let res: Result<ManageWorkItemLinkParams, _> = serde_json::from_value(payload);
        assert!(res.is_err());
        let err = res.err().unwrap().to_string();
        assert!(err.contains("found top-level"));
        assert!(err.contains("update_work_item"));
    }

    #[tokio::test]
    async fn manage_work_item_link_url_and_external_types() {
        let h = test_handler();

        let remote_url = "https://dev.azure.com/otherorg/_apis/wit/workItems/350".to_string();
        let params = ManageWorkItemLinkParams {
            action: "add".to_string(),
            source_id: WorkItemId::Int(300),
            target: WorkItemId::Str(remote_url.clone()),
            link_type: Some("System.LinkTypes.Remote.Related".to_string()),
            attributes: None,
        };
        let res = h.manage_work_item_link(Parameters(params)).await.unwrap();
        assert!(res.0.success);
        let rels = res.0.relations.unwrap_or_default();
        assert!(
            rels.iter()
                .any(|r| r.rel == "System.LinkTypes.Remote.Related" && r.url == remote_url)
        );

        let bad_target =
            "https://tfs.example.com/tfs/DefaultCollection/_workitems/edit/9999999".to_string();
        let bad_params = ManageWorkItemLinkParams {
            action: "add".to_string(),
            source_id: WorkItemId::Int(302),
            target: WorkItemId::Str(bad_target),
            link_type: Some("System.LinkTypes.Remote.Related".to_string()),
            attributes: None,
        };
        let bad_res = h.manage_work_item_link(Parameters(bad_params)).await;
        assert!(bad_res.is_err());
        let bad_err = bad_res.err().unwrap();
        assert!(bad_err.contains("suspicious target URL"));

        let web_url = "https://example.com/docs/guide".to_string();
        let params2 = ManageWorkItemLinkParams {
            action: "add".to_string(),
            source_id: WorkItemId::Int(301),
            target: WorkItemId::Str(web_url.clone()),
            link_type: Some("Hyperlink".to_string()),
            attributes: None,
        };
        let res2 = h.manage_work_item_link(Parameters(params2)).await.unwrap();
        assert!(res2.0.success);
        let rels2 = res2.0.relations.unwrap_or_default();
        assert!(
            rels2
                .iter()
                .any(|r| r.rel == "Hyperlink" && r.url == web_url)
        );
    }

    #[tokio::test]
    async fn manage_work_item_link_wraps_azure_missing_fields_error() {
        use crate::test_helpers::missing_fields_client;
        let bad = missing_fields_client();
        let h = AdoMcpHandler::new(
            WorkItemOperations::new(bad.clone()),
            LinkOperations::new(bad.clone()),
            CommentOperations::new(bad.clone()),
            ProjectLookup::new(bad.clone()),
            QueryOperations::new(bad),
        );
        let params = ManageWorkItemLinkParams {
            action: "add".to_string(),
            source_id: WorkItemId::Int(400),
            target: WorkItemId::Int(401),
            link_type: Some("System.LinkTypes.Related".to_string()),
            attributes: None,
        };

        let res = h.manage_work_item_link(Parameters(params)).await;
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert!(err.contains("missing field `fields`"));
        assert!(err.contains("To add a relation to an existing work item"));
    }

    #[tokio::test]
    async fn update_work_item_tags_read_and_modify() {
        let h = test_handler();

        let add_patch = serde_json::json!([
            {"op":"add","path":"/fields/System.Tags","value":"urgent;ops"}
        ]);
        let params = UpdateWorkItemParams {
            id: WorkItemId::Int(400),
            patch: Some(add_patch),
            tags: None,
            expected_rev: None,
        };
        let res = h.update_work_item(Parameters(params)).await.unwrap();
        let tags_val = res
            .0
            .fields
            .extra
            .as_ref()
            .and_then(|m| m.get("System.Tags"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(tags_val, "urgent;ops");

        let remove_patch = serde_json::json!([
            {"op":"remove","path":"/fields/System.Tags"}
        ]);
        let params2 = UpdateWorkItemParams {
            id: WorkItemId::Int(400),
            patch: Some(remove_patch),
            tags: None,
            expected_rev: None,
        };
        let res2 = h.update_work_item(Parameters(params2)).await.unwrap();
        let tags_val2 = res2
            .0
            .fields
            .extra
            .as_ref()
            .and_then(|m| m.get("System.Tags"))
            .and_then(|v| v.as_str());
        assert!(tags_val2.is_none());
    }

    #[tokio::test]
    async fn create_and_manage_custom_date_fields() {
        let h = test_handler();

        let mut extra_map = std::collections::BTreeMap::new();
        extra_map.insert(
            "Custom.EstimationExpectedDate".to_string(),
            serde_json::Value::String("2026-03-01".to_string()),
        );
        let params = CreateWorkItemParams {
            project: "Demo".into(),
            wi_type: "Task".into(),
            fields: WorkItemFields {
                title: Some("Estimate due date".into()),
                description: None,
                extra: Some(extra_map),
            },
            relations: None,
            comments: None,
        };
        let res = h.create_work_item(Parameters(params)).await.unwrap();
        let est = res
            .0
            .fields
            .extra
            .as_ref()
            .and_then(|m| m.get("Custom.EstimationExpectedDate"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(est, "2026-03-01");

        let update_patch = serde_json::json!([
            {"op":"replace","path":"/fields/Custom.EstimationExpectedDate","value":"2026-03-15"}
        ]);
        let up_params = UpdateWorkItemParams {
            id: WorkItemId::Int(123),
            patch: Some(update_patch),
            tags: None,
            expected_rev: None,
        };
        let res2 = h.update_work_item(Parameters(up_params)).await.unwrap();
        let est2 = res2
            .0
            .fields
            .extra
            .as_ref()
            .and_then(|m| m.get("Custom.EstimationExpectedDate"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(est2, "2026-03-15");

        let remove_patch = serde_json::json!([
            {"op":"remove","path":"/fields/Custom.EstimationExpectedDate"}
        ]);
        let rm_params = UpdateWorkItemParams {
            id: WorkItemId::Int(123),
            patch: Some(remove_patch),
            tags: None,
            expected_rev: None,
        };
        let res3 = h.update_work_item(Parameters(rm_params)).await.unwrap();
        let est3 = res3
            .0
            .fields
            .extra
            .as_ref()
            .and_then(|m| m.get("Custom.EstimationExpectedDate"))
            .and_then(|v| v.as_str());
        assert!(est3.is_none());
    }
}
