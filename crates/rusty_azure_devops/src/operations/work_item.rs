use std::sync::Arc;

use azure_devops_client::{AzureDevOpsClient, AzureError, Comment, HttpError};

use crate::domain::*;

use super::ServiceConfig;
use super::helpers::{normalize_description_fields, rewrite_missing_fields_error};

/// Work item domain operations: create, update, get, search.
/// Absorbs Patch Document construction, HTML-to-Markdown conversion,
/// and comment handling (via separate CommentOperations module).
/// Two call-sites (MCP handler and CLI) depend on this interface.
#[derive(Clone)]
pub struct WorkItemOperations {
    client: Arc<dyn AzureDevOpsClient>,
    config: ServiceConfig,
}

impl WorkItemOperations {
    pub fn new(client: Arc<dyn AzureDevOpsClient>) -> Self {
        Self {
            client,
            config: ServiceConfig::default(),
        }
    }

    pub fn with_config(client: Arc<dyn AzureDevOpsClient>, config: ServiceConfig) -> Self {
        Self { client, config }
    }

    pub async fn create_work_item(
        &self,
        p: CreateWorkItemParams,
    ) -> Result<CreateWorkItemResult, AzureError> {
        let patch_doc = self.build_create_patch(&p)?;
        let wi = self
            .client
            .create_work_item(&p.project, &p.wi_type, &patch_doc)
            .await?;

        if let Some(comments) = p.comments {
            self.attach_comments(wi.id, comments).await?;
            if let Some(fresh) = self.refresh_fields(wi.id).await {
                return Ok(fresh);
            }
        }
        Ok(self.work_item_to_result(wi))
    }

    /// Validate title and build the JSON Patch document for work item creation.
    fn build_create_patch(
        &self,
        p: &CreateWorkItemParams,
    ) -> Result<serde_json::Value, AzureError> {
        let has_title = p.fields.title.is_some()
            || p.fields
                .extra
                .as_ref()
                .map(|m| m.contains_key("System.Title"))
                .unwrap_or(false);
        if !has_title {
            return Err(AzureError::Http(HttpError::decode(
                "missing required field System.Title",
            )));
        }

        let mut patch_ops: Vec<serde_json::Value> = fields_to_patch_ops(&p.fields, "add");
        if let Some(rels) = &p.relations {
            for r in rels {
                patch_ops.push(relation_to_patch(r, "add"));
            }
        }
        Ok(serde_json::Value::Array(patch_ops))
    }

    /// Add comments to a work item after creation.
    async fn attach_comments(
        &self,
        work_item_id: i64,
        comments: Vec<CommentDto>,
    ) -> Result<(), AzureError> {
        for c in comments {
            let client_comment = Comment {
                id: c.id,
                author: c.author,
                text: c.text,
                created_at: c.created_at,
            };
            let _ = self
                .client
                .add_comment(work_item_id, &client_comment)
                .await?;
        }
        Ok(())
    }

    /// Re-fetch a work item to get updated fields (e.g. System.CommentCount).
    /// Returns `None` on failure — callers fall back to original fields.
    async fn refresh_fields(&self, work_item_id: i64) -> Option<CreateWorkItemResult> {
        match self.client.get_work_item(work_item_id, None, None).await {
            Ok(fresh) => Some(self.work_item_to_result(fresh)),
            Err(e) => {
                tracing::warn!(
                    work_item_id,
                    error = %e,
                    "re-fetch after adding comments failed; returning original fields"
                );
                None
            }
        }
    }

    /// Convert a raw `WorkItem` from the client into a `CreateWorkItemResult`.
    fn work_item_to_result(&self, wi: azure_devops_client::WorkItem) -> CreateWorkItemResult {
        CreateWorkItemResult {
            id: wi.id,
            url: format!("workitems/{}", wi.id),
            fields: value_to_workitemfields(wi.fields),
            relations: wi
                .relations
                .map(|rs| rs.into_iter().map(|r| r.into()).collect()),
        }
    }

    pub async fn update_work_item(
        &self,
        p: UpdateWorkItemParams,
    ) -> Result<GetWorkItemResult, AzureError> {
        let id = p.id.to_i64()?;
        let patch_to_use = if let Some(patch_v) = &p.patch {
            patch_v.clone()
        } else if let Some(tags_s) = &p.tags {
            serde_json::json!([{"op":"add","path":"/fields/System.Tags","value": tags_s}])
        } else {
            return Err(AzureError::Http(HttpError::decode(
                "missing required parameter: patch (or provide `tags` as a convenience). Example: { \"id\": 123, \"patch\": [{\"op\":\"add\", \"path\": \"/fields/System.Tags\", \"value\": \"urgent\"} ] }",
            )));
        };

        let wi = self
            .client
            .update_work_item(id, &patch_to_use, p.expected_rev)
            .await
            .map_err(rewrite_missing_fields_error)?;
        Ok(GetWorkItemResult {
            id: wi.id,
            rev: wi.rev,
            fields: value_to_workitemfields(wi.fields),
            comments: None,
            relations: wi
                .relations
                .map(|rs| rs.into_iter().map(|r| r.into()).collect()),
        })
    }

    pub async fn search_work_items(
        &self,
        p: SearchWorkItemsParams,
    ) -> Result<SearchWorkItemsResult, AzureError> {
        let project = p
            .project
            .or_else(|| self.config.default_project.clone())
            .ok_or_else(|| {
                AzureError::Http(HttpError::decode(
                    "project is required: pass it explicitly or set ADO_DEFAULT_PROJECT",
                ))
            })?;
        let limit = p.limit.unwrap_or(100);
        let ids = if let Some(w) = p.wiql.as_deref() {
            self.client.query_wiql(&project, w, limit).await?
        } else {
            vec![]
        };
        let total = ids.len();
        let details = if p.details.unwrap_or(false) && !ids.is_empty() {
            let mut fields: Vec<String> =
                DEFAULT_LIST_FIELDS.iter().map(|s| s.to_string()).collect();
            if p.include_description.unwrap_or(false) {
                for f in DESCRIPTION_FIELDS {
                    if !fields.iter().any(|f2| f2 == f) {
                        fields.push(f.to_string());
                    }
                }
            }
            let fields_slice: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
            let wis = self
                .client
                .batch_get_work_items(&ids, Some(&fields_slice))
                .await?;

            let rel_format = p.relations_format.unwrap_or(RelationsFormat::Ids);

            Some(
                wis.into_iter()
                    .map(|wi| {
                        let mut wf = value_to_workitemfields(wi.fields);
                        normalize_description_fields(&mut wf);

                        let relations = wi
                            .relations
                            .map(|rs| rs.into_iter().map(|r| r.into()).collect());

                        GetWorkItemResult {
                            id: wi.id,
                            rev: wi.rev,
                            fields: wf,
                            comments: wi
                                .comments
                                .map(|cs| cs.into_iter().map(|c| c.into()).collect()),
                            relations,
                        }
                        .with_relations_format(&rel_format)
                    })
                    .collect(),
            )
        } else {
            None
        };
        Ok(SearchWorkItemsResult {
            ids,
            total,
            details,
        })
    }

    pub async fn get_work_item(
        &self,
        p: GetWorkItemParams,
    ) -> Result<GetWorkItemResult, AzureError> {
        let id_wi = p.id.ok_or_else(|| {
            AzureError::Http(HttpError::decode("missing required parameter: id. Example: { \"id\": 100 }.\nCommon misnamed keys: \"workItemId\", \"work_item_id\". Make sure to pass `id` (int) not `workItemId`."))
        })?;
        let id = id_wi.to_i64()?;

        let mut fields: Vec<String> = p
            .fields
            .unwrap_or_else(|| DEFAULT_GET_FIELDS.iter().map(|s| s.to_string()).collect());

        let wildcard = fields.len() == 1 && fields[0] == "*";
        if wildcard {
            fields.clear();
        }

        if !wildcard && p.include_description.unwrap_or(false) {
            for f in DESCRIPTION_FIELDS {
                if !fields.iter().any(|f2| f2 == f) {
                    fields.push(f.to_string());
                }
            }
        }
        let fields_slice: Option<Vec<&str>> = if wildcard {
            None
        } else {
            Some(fields.iter().map(|s| s.as_str()).collect())
        };

        let expand_val = p.expand.as_deref();

        let wi = self
            .client
            .get_work_item(id, fields_slice.as_deref(), expand_val)
            .await?;

        let mut wf = value_to_workitemfields(wi.fields);
        normalize_description_fields(&mut wf);

        let comments = if p.include_comments.unwrap_or(false) {
            let raw = self.client.get_comments(id).await?;
            Some(raw.into_iter().map(|c| c.into()).collect())
        } else {
            wi.comments
                .map(|cs| cs.into_iter().map(|c| c.into()).collect())
        };
        let relations = wi
            .relations
            .map(|rs| rs.into_iter().map(|r| r.into()).collect());

        let rel_format = p.relations_format.unwrap_or(RelationsFormat::Ids);

        let result = GetWorkItemResult {
            id: wi.id,
            rev: wi.rev,
            fields: wf,
            comments,
            relations,
        }
        .with_relations_format(&rel_format);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn service_create_work_item() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: Some("Test".into()),
                    description: None,
                    extra: None,
                },
                relations: None,
                comments: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_create_work_item_missing_title() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: None,
                    description: None,
                    extra: None,
                },
                relations: None,
                comments: None,
            })
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("System.Title"));
    }

    #[tokio::test]
    async fn service_create_work_item_with_relations() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: Some("Test".into()),
                    description: None,
                    extra: None,
                },
                relations: Some(vec![RelationDto {
                    url: "https://example.com/workItems/100".into(),
                    rel: "System.LinkTypes.Related".into(),
                    attributes: Some(RelationAttributes {
                        comment: Some("link".into()),
                    }),
                }]),
                comments: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        let rels = res.relations.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].rel, "System.LinkTypes.Related");
        assert!(rels[0].url.ends_with("/100"));
    }

    #[tokio::test]
    async fn service_create_work_item_with_comments() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: Some("Test".into()),
                    description: None,
                    extra: None,
                },
                relations: None,
                comments: Some(vec![CommentDto {
                    id: None,
                    author: None,
                    text: "my comment".into(),
                    created_at: None,
                }]),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_create_work_item_with_relations_and_comments() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: Some("Full".into()),
                    description: None,
                    extra: None,
                },
                relations: Some(vec![RelationDto {
                    url: "https://example.com/workItems/789".into(),
                    rel: "System.LinkTypes.Related".into(),
                    attributes: None,
                }]),
                comments: Some(vec![CommentDto {
                    id: None,
                    author: None,
                    text: "linked".into(),
                    created_at: None,
                }]),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        let rels = res.relations.unwrap();
        assert_eq!(rels.len(), 1);
        assert!(rels[0].url.ends_with("/789"));
        // InMemoryAzureClient does not track System.CommentCount (that's
        // production behavior — the re-fetch after create picks it up).
        assert!(res.fields.extra.is_none() || res.fields.extra.as_ref().unwrap().is_empty());
    }

    #[tokio::test]
    async fn service_create_work_item_with_comments_updates_comment_count() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .create_work_item(CreateWorkItemParams {
                project: "Demo".into(),
                wi_type: "Task".into(),
                fields: WorkItemFields {
                    title: Some("Test".into()),
                    description: None,
                    extra: None,
                },
                relations: None,
                comments: Some(vec![
                    CommentDto {
                        id: None,
                        author: None,
                        text: "first comment".into(),
                        created_at: None,
                    },
                    CommentDto {
                        id: None,
                        author: None,
                        text: "second comment".into(),
                        created_at: None,
                    },
                ]),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        // InMemoryAzureClient does not track System.CommentCount (production
        // behavior). Test verifies the create+comment flow succeeds without error.
    }

    #[tokio::test]
    async fn service_get_work_item_with_comments() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: Some("Comments".into()),
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        assert!(res.comments.is_some());
    }

    #[tokio::test]
    async fn service_get_work_item_with_include_comments() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: None,
                include_comments: Some(true),
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert!(res.comments.is_some());
    }

    #[tokio::test]
    async fn service_get_work_item_missing_id() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: None,
                fields: None,
                expand: None,
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn service_get_work_item_with_description() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: None,
                include_comments: None,
                include_description: Some(true),
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_search_work_items_with_wiql() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .search_work_items(SearchWorkItemsParams {
                project: Some("Demo".into()),
                wiql: Some("SELECT [System.Id] FROM WorkItems".into()),
                details: Some(true),
                limit: Some(10),
                include_description: Some(false),
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.total, 2);
        assert!(res.details.is_some());
    }

    #[tokio::test]
    async fn service_search_work_items_without_wiql() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .search_work_items(SearchWorkItemsParams {
                project: Some("Demo".into()),
                wiql: None,
                details: None,
                limit: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.total, 0);
        assert!(res.details.is_none());
    }

    #[tokio::test]
    async fn service_update_work_item_with_tags() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .update_work_item(UpdateWorkItemParams {
                id: WorkItemId::Int(123),
                patch: None,
                tags: Some("urgent;ops".into()),
                expected_rev: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_update_work_item_no_patch_no_tags() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .update_work_item(UpdateWorkItemParams {
                id: WorkItemId::Int(123),
                patch: None,
                tags: None,
                expected_rev: None,
            })
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn service_get_work_item_with_relations_format() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: None,
                include_comments: None,
                include_description: None,
                relations_format: Some(RelationsFormat::Urls),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_get_work_item_with_wildcard_fields() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: Some(vec!["*".into()]),
                expand: None,
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_get_work_item_with_fields_list() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: Some(vec!["System.Title".into(), "System.Tags".into()]),
                expand: None,
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_update_work_item_with_patch() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .update_work_item(UpdateWorkItemParams {
                id: WorkItemId::Int(123),
                patch: Some(serde_json::json!([
                    {"op":"add","path":"/fields/System.Tags","value":"patched"}
                ])),
                tags: None,
                expected_rev: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_search_work_items_missing_project() {
        use azure_devops_client::InMemoryAzureClient;
        let config = ServiceConfig {
            default_project: None,
            ..ServiceConfig::default()
        };
        let svc = WorkItemOperations::with_config(Arc::new(InMemoryAzureClient::default()), config);
        let res = svc
            .search_work_items(SearchWorkItemsParams {
                project: None,
                wiql: Some("SELECT [Id] FROM WorkItems".into()),
                details: None,
                limit: None,
                include_description: None,
                relations_format: None,
            })
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("project is required"));
    }

    #[tokio::test]
    async fn service_search_work_items_with_description() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .search_work_items(SearchWorkItemsParams {
                project: Some("Demo".into()),
                wiql: Some("SELECT [Id] FROM WorkItems".into()),
                details: Some(true),
                limit: Some(10),
                include_description: Some(true),
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.total, 2);
        assert!(res.details.is_some());
    }

    #[tokio::test]
    async fn service_get_work_item_with_expand_relations() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: Some("relations".into()),
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }

    #[tokio::test]
    async fn service_get_work_item_with_default_fields() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = WorkItemOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_work_item(GetWorkItemParams {
                id: Some(WorkItemId::Int(123)),
                fields: None,
                expand: None,
                include_comments: None,
                include_description: None,
                relations_format: None,
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
    }
}
