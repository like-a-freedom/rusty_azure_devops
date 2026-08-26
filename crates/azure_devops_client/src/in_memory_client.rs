use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{AzureDevOpsClient, AzureError, Comment, Project, Relation, WorkItem};

/// In-memory test adapter for `AzureDevOpsClient`. Stores state in plain
/// collections; reads and writes happen directly on those collections.
/// Minimal patch logic: `create_work_item` extracts fields/relations from
/// the patch; `update_work_item` applies add/replace/remove ops in-memory.
///
/// Each method supports optional error injection through per-method error
/// fields on the struct. Sensible defaults are populated via `Default`.
#[derive(Clone)]
pub struct InMemoryAzureClient {
    // State
    pub projects: Vec<Project>,
    pub work_items: Arc<Mutex<HashMap<i64, WorkItem>>>,
    pub comments: Arc<Mutex<HashMap<i64, Vec<Comment>>>>,
    pub query_result: Vec<i64>,

    // Per-method error injection
    pub list_projects_error: Option<AzureError>,
    pub create_work_item_error: Option<AzureError>,
    pub get_work_item_error: Option<AzureError>,
    pub update_work_item_error: Option<AzureError>,
    pub query_wiql_error: Option<AzureError>,
    pub batch_get_work_items_error: Option<AzureError>,
    pub get_comments_error: Option<AzureError>,
    pub add_comment_error: Option<AzureError>,
}

impl Default for InMemoryAzureClient {
    fn default() -> Self {
        let mut work_items = HashMap::new();
        work_items.insert(
            123,
            WorkItem {
                id: 123,
                rev: Some(1),
                fields: serde_json::json!({"System.Title": "title", "System.CommentCount": 0}),
                comments: None,
                relations: Some(vec![Relation {
                    rel: Some("System.LinkTypes.Related".to_string()),
                    url: "https://example.com/workItems/124".to_string(),
                    attributes: None,
                }]),
            },
        );

        let mut comments = HashMap::new();
        comments.insert(
            123,
            vec![Comment {
                id: Some(1),
                author: Some("user".to_string()),
                text: "hello".to_string(),
                created_at: None,
            }],
        );

        Self {
            projects: vec![Project {
                id: "1".to_string(),
                name: "Demo".to_string(),
            }],
            work_items: Arc::new(Mutex::new(work_items)),
            comments: Arc::new(Mutex::new(comments)),
            query_result: vec![123, 456],
            list_projects_error: None,
            create_work_item_error: None,
            get_work_item_error: None,
            update_work_item_error: None,
            query_wiql_error: None,
            batch_get_work_items_error: None,
            get_comments_error: None,
            add_comment_error: None,
        }
    }
}

impl InMemoryAzureClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Arc<dyn AzureDevOpsClient> {
        Arc::new(self)
    }

    fn apply_patch_to_fields(
        &self,
        fields_map: &mut serde_json::Map<String, serde_json::Value>,
        relations: &mut Vec<Relation>,
        patch: &serde_json::Value,
    ) {
        if let Some(arr) = patch.as_array() {
            for op in arr.iter() {
                let op_name = match op.get("op").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => continue,
                };
                match op_name {
                    "add" | "replace" => {
                        if let Some(path) = op.get("path").and_then(|v| v.as_str())
                            && let Some(val) = op.get("value")
                        {
                            if path.starts_with("/relations") {
                                let rel = Relation {
                                    rel: val
                                        .get("rel")
                                        .and_then(|r| r.as_str().map(|s| s.to_string())),
                                    url: val
                                        .get("url")
                                        .and_then(|u| u.as_str().map(|s| s.to_string()))
                                        .unwrap_or_default(),
                                    attributes: val.get("attributes").cloned(),
                                };
                                relations.push(rel);
                            } else if let Some(field) = path.strip_prefix("/fields/") {
                                fields_map.insert(field.to_string(), val.clone());
                            }
                        }
                    }
                    "remove" => {
                        if let Some(path) = op.get("path").and_then(|v| v.as_str())
                            && let Some(idx_str) = path.strip_prefix("/relations/")
                            && let Ok(idx) = idx_str.parse::<usize>()
                            && idx < relations.len()
                        {
                            relations.remove(idx);
                        } else if let Some(path) = op.get("path").and_then(|v| v.as_str())
                            && let Some(field) = path.strip_prefix("/fields/")
                        {
                            fields_map.remove(field);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl AzureDevOpsClient for InMemoryAzureClient {
    async fn list_projects(&self) -> Result<Vec<Project>, AzureError> {
        if let Some(e) = &self.list_projects_error {
            return Err(e.clone());
        }
        Ok(self.projects.clone())
    }

    async fn create_work_item(
        &self,
        _project: &str,
        _wi_type: &str,
        patch: &serde_json::Value,
    ) -> Result<WorkItem, AzureError> {
        if let Some(e) = &self.create_work_item_error {
            return Err(e.clone());
        }
        let mut fields_map = serde_json::Map::new();
        let mut relations = Vec::<Relation>::new();
        self.apply_patch_to_fields(&mut fields_map, &mut relations, patch);
        let id = 123;
        let wi = WorkItem {
            id,
            rev: Some(1),
            fields: serde_json::Value::Object(fields_map),
            comments: None,
            relations: if relations.is_empty() {
                None
            } else {
                Some(relations)
            },
        };
        self.work_items.lock().unwrap().insert(id, wi.clone());
        Ok(wi)
    }

    async fn get_work_item(
        &self,
        id: i64,
        _fields: Option<&[&str]>,
        expand: Option<&str>,
    ) -> Result<WorkItem, AzureError> {
        if let Some(e) = &self.get_work_item_error {
            return Err(e.clone());
        }
        let mut result = self
            .work_items
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .unwrap_or(WorkItem {
                id,
                rev: Some(1),
                fields: serde_json::json!({"System.Title": "title"}),
                comments: None,
                relations: None,
            });
        let expand_str = expand.unwrap_or("");
        if (expand_str.eq_ignore_ascii_case("Comments") || expand_str.eq_ignore_ascii_case("All"))
            && result.comments.is_none()
        {
            let comments = self.comments.lock().unwrap();
            result.comments = Some(comments.get(&id).cloned().unwrap_or_default());
        }
        Ok(result)
    }

    async fn update_work_item(
        &self,
        id: i64,
        patch: &serde_json::Value,
        _expected_rev: Option<u64>,
    ) -> Result<WorkItem, AzureError> {
        if let Some(e) = &self.update_work_item_error {
            return Err(e.clone());
        }
        let mut wi = self
            .work_items
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .unwrap_or(WorkItem {
                id,
                rev: Some(2),
                fields: serde_json::json!({"System.Title": "updated"}),
                comments: None,
                relations: Some(vec![Relation {
                    rel: Some("System.LinkTypes.Related".to_string()),
                    url: format!("https://example.com/workItems/{}", id + 1),
                    attributes: None,
                }]),
            });

        let mut fields_map = match wi.fields.as_object().cloned() {
            Some(m) => m,
            None => serde_json::Map::new(),
        };
        let mut relations = wi.relations.clone().unwrap_or_default();
        self.apply_patch_to_fields(&mut fields_map, &mut relations, patch);
        wi.fields = serde_json::Value::Object(fields_map);
        wi.relations = Some(relations);
        wi.rev = Some(wi.rev.unwrap_or(0) + 1);
        Ok(wi)
    }

    async fn query_wiql(
        &self,
        _project: &str,
        _wiql: &str,
        _limit: usize,
    ) -> Result<Vec<i64>, AzureError> {
        if let Some(e) = &self.query_wiql_error {
            return Err(e.clone());
        }
        Ok(self.query_result.clone())
    }

    async fn query_by_id(&self, _project: &str, _query_id: &str) -> Result<Vec<i64>, AzureError> {
        if let Some(e) = &self.query_wiql_error {
            return Err(e.clone());
        }
        Ok(self.query_result.clone())
    }

    async fn batch_get_work_items(
        &self,
        ids: &[i64],
        _fields: Option<&[&str]>,
    ) -> Result<Vec<WorkItem>, AzureError> {
        if let Some(e) = &self.batch_get_work_items_error {
            return Err(e.clone());
        }
        Ok(ids
            .iter()
            .filter_map(|id| {
                self.work_items
                    .lock()
                    .unwrap()
                    .get(id)
                    .cloned()
                    .or(Some(WorkItem {
                        id: *id,
                        rev: Some(1),
                        fields: serde_json::json!({"System.Title": "title"}),
                        comments: None,
                        relations: None,
                    }))
            })
            .collect())
    }

    async fn get_comments(&self, work_item_id: i64) -> Result<Vec<Comment>, AzureError> {
        if let Some(e) = &self.get_comments_error {
            return Err(e.clone());
        }
        let comments = self.comments.lock().unwrap();
        Ok(comments.get(&work_item_id).cloned().unwrap_or_default())
    }

    async fn add_comment(
        &self,
        work_item_id: i64,
        comment: &Comment,
    ) -> Result<Comment, AzureError> {
        if let Some(e) = &self.add_comment_error {
            return Err(e.clone());
        }
        let mut comments = self.comments.lock().unwrap();
        let result = Comment {
            id: Some((comments.get(&work_item_id).map_or(0, |v| v.len()) as i64) + 1),
            author: comment.author.clone(),
            text: comment.text.clone(),
            created_at: comment.created_at.clone(),
        };
        comments
            .entry(work_item_id)
            .or_default()
            .push(result.clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_populates_sensible_state() {
        let c = InMemoryAzureClient::default();
        let projects = c.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Demo");
    }

    #[tokio::test]
    async fn get_work_item_expands_comments() {
        let c = InMemoryAzureClient::default();
        let wi = c.get_work_item(123, None, Some("Comments")).await.unwrap();
        assert!(wi.comments.is_some());
        assert_eq!(wi.comments.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn update_work_item_applies_patch() {
        let c = InMemoryAzureClient::default();
        let patch = serde_json::json!([
            {"op":"add","path":"/fields/System.Tags","value":"urgent"}
        ]);
        let wi = c.update_work_item(456, &patch, None).await.unwrap();
        let tags = wi
            .fields
            .get("System.Tags")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(tags, "urgent");
    }

    #[tokio::test]
    async fn error_injection_per_method() {
        use crate::HttpError;
        let c = InMemoryAzureClient {
            update_work_item_error: Some(AzureError::Http(HttpError::Conflict)),
            ..Default::default()
        };
        let err = c
            .update_work_item(1, &serde_json::json!([]), None)
            .await
            .unwrap_err();
        assert!(matches!(err, AzureError::Http(HttpError::Conflict)));
    }

    #[tokio::test]
    async fn build_returns_arc_dyn() {
        let _c: Arc<dyn AzureDevOpsClient> = InMemoryAzureClient::default().build();
    }
}
