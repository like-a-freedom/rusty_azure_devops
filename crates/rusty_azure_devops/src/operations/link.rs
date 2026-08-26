use std::sync::Arc;

use azure_devops_client::{AzureDevOpsClient, AzureError, HttpError};

use crate::domain::*;

use super::ServiceConfig;
use super::helpers::rewrite_missing_fields_error;

/// Manages relations between work items — add and remove.
/// Holds `ServiceConfig` because link construction needs `base_url`
/// to build API URLs from integer target IDs.
#[derive(Clone)]
pub struct LinkOperations {
    client: Arc<dyn AzureDevOpsClient>,
    config: ServiceConfig,
}

impl LinkOperations {
    pub fn new(client: Arc<dyn AzureDevOpsClient>) -> Self {
        Self {
            client,
            config: ServiceConfig::default(),
        }
    }

    pub fn with_config(client: Arc<dyn AzureDevOpsClient>, config: ServiceConfig) -> Self {
        Self { client, config }
    }

    pub async fn manage_work_item_link(
        &self,
        p: ManageWorkItemLinkParams,
    ) -> Result<ManageWorkItemLinkResult, AzureError> {
        let action = p.action.to_lowercase();
        let src_id = p.source_id.to_i64()?;
        let target_url = match &p.target {
            WorkItemId::Int(tid) => {
                let base = self
                    .config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://example.com");
                format!("{}/_apis/wit/workItems/{}", base.trim_end_matches('/'), tid)
            }
            WorkItemId::Str(s) => {
                if s.starts_with("http://") || s.starts_with("https://") {
                    s.clone()
                } else if let Ok(tid) = s.parse::<i64>() {
                    let base = self
                        .config
                        .base_url
                        .as_deref()
                        .unwrap_or("https://example.com");
                    format!("{}/_apis/wit/workItems/{}", base.trim_end_matches('/'), tid)
                } else {
                    return Err(AzureError::Http(HttpError::decode(
                        "target must be an integer work item id or a URL",
                    )));
                }
            }
        };
        let link_type = p
            .link_type
            .unwrap_or_else(|| "System.LinkTypes.Related".to_string());

        if target_url.contains("/_workitems/edit/") {
            return Err(AzureError::Http(HttpError::decode(format!(
                "suspicious target URL: detected a web 'edit' page URL ({}). Use the numeric target id or the API URL '/_apis/wit/workItems/<id>' instead. Example: {{\"action\":\"add\", \"source_id\": 100, \"target\": 200, \"link_type\": \"System.LinkTypes.Related\"}}",
                target_url
            ))));
        }

        match action.as_str() {
            "add" => {
                let dto = RelationDto {
                    url: target_url,
                    rel: link_type,
                    attributes: p.attributes.map(|v| {
                        let comment = v
                            .get("comment")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string());
                        crate::domain::RelationAttributes { comment }
                    }),
                };
                let patch = serde_json::json!([relation_to_patch(&dto, "add")]);
                let wi = self
                    .client
                    .update_work_item(src_id, &patch, None)
                    .await
                    .map_err(rewrite_missing_fields_error)?;
                let relations = wi
                    .relations
                    .map(|rs| rs.into_iter().map(|r| r.into()).collect());
                Ok(ManageWorkItemLinkResult {
                    success: true,
                    message: None,
                    relations,
                })
            }
            "remove" => {
                let wi = self
                    .client
                    .get_work_item(src_id, None, Some("relations"))
                    .await?;
                let relations = wi
                    .relations
                    .ok_or_else(|| AzureError::Http(HttpError::decode("no relations found")))?;
                let target_str = target_url.clone();
                let target_id_opt = p.target.to_i64().ok();
                let mut found_idx: Option<usize> = None;
                for (i, r) in relations.iter().enumerate() {
                    if r.url == target_str
                        || target_id_opt
                            .map(|id| r.url.ends_with(&format!("/{}", id)))
                            .unwrap_or(false)
                    {
                        found_idx = Some(i);
                        break;
                    }
                }
                let idx = found_idx
                    .ok_or_else(|| AzureError::Http(HttpError::decode("relation not found")))?;
                let path = format!("/relations/{}", idx);
                let patch = serde_json::json!([{"op":"remove","path": path}]);
                let wi2 = self.client.update_work_item(src_id, &patch, None).await?;
                let relations = wi2
                    .relations
                    .map(|rs| rs.into_iter().map(|r| r.into()).collect());
                Ok(ManageWorkItemLinkResult {
                    success: true,
                    message: None,
                    relations,
                })
            }
            _ => Err(AzureError::Http(HttpError::decode(
                "unsupported action; must be 'add' or 'remove'. Example add: {\"action\":\"add\", \"source_id\": 100, \"target\": 101, \"link_type\": \"System.LinkTypes.Related\"}. Common misnamed keys: \"relationType\", \"comment\".",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn link_ops_add_with_attributes() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "add".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Int(200),
                link_type: Some("System.LinkTypes.Related".into()),
                attributes: Some(serde_json::json!({"comment": "test"})),
            })
            .await
            .unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn link_ops_add_without_link_type() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "add".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Int(200),
                link_type: None,
                attributes: None,
            })
            .await
            .unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn link_ops_remove() {
        use azure_devops_client::InMemoryAzureClient;
        let client = InMemoryAzureClient::default();
        {
            let mut work_items = client.work_items.lock().unwrap();
            work_items.insert(
                100,
                azure_devops_client::WorkItem {
                    id: 100,
                    rev: Some(1),
                    fields: serde_json::json!({"System.Title": "src"}),
                    comments: None,
                    relations: Some(vec![azure_devops_client::Relation {
                        rel: Some("System.LinkTypes.Related".to_string()),
                        url: "https://example.com/workItems/101".to_string(),
                        attributes: None,
                    }]),
                },
            );
        }
        let svc = LinkOperations::new(Arc::new(client));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "remove".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Int(101),
                link_type: None,
                attributes: None,
            })
            .await
            .unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn link_ops_unsupported_action() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "invalid".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Int(200),
                link_type: None,
                attributes: None,
            })
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn link_ops_suspicious_url() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "add".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Str(
                    "https://tfs.example.com/tfs/DefaultCollection/_workitems/edit/200".into(),
                ),
                link_type: None,
                attributes: None,
            })
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("suspicious"));
    }

    #[tokio::test]
    async fn link_ops_target_string_id() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "add".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Str("42".into()),
                link_type: None,
                attributes: None,
            })
            .await
            .unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn link_ops_target_invalid_string() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "add".into(),
                source_id: WorkItemId::Int(100),
                target: WorkItemId::Str("not-a-number-or-url".into()),
                link_type: None,
                attributes: None,
            })
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn link_ops_remove_not_found() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = LinkOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .manage_work_item_link(ManageWorkItemLinkParams {
                action: "remove".into(),
                source_id: WorkItemId::Int(999),
                target: WorkItemId::Int(888),
                link_type: None,
                attributes: None,
            })
            .await;
        assert!(res.is_err());
    }
}
