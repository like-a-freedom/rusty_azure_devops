use azure_devops_client::{AzureError, HttpError};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// WorkItemId — flexible input type accepting int or string
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[serde(untagged)]
pub enum WorkItemId {
    Int(i64),
    Str(String),
}

impl WorkItemId {
    pub fn to_i64(&self) -> Result<i64, AzureError> {
        match self {
            WorkItemId::Int(i) => Ok(*i),
            WorkItemId::Str(s) => s.parse::<i64>().map_err(|_| {
                AzureError::Http(HttpError::decode(format!(
                    "invalid id: {} (expected integer)",
                    s
                )))
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// WorkItemFields — common fields + extra bag
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkItemFields {
    #[serde(rename = "System.Title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "System.Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub extra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// Relation DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationDto {
    pub url: String,
    pub rel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<RelationAttributes>,
}

impl RelationDto {
    /// Extract the numeric work item ID from a relation URL.
    /// URLs follow the pattern: `.../_apis/wit/workItems/<id>`
    pub fn work_item_id_from_url(url: &str) -> Option<i64> {
        url.rsplit('/').next()?.parse::<i64>().ok()
    }
}

// ---------------------------------------------------------------------------
// Comment DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommentDto {
    pub id: Option<i64>,
    pub author: Option<String>,
    pub text: String,
    pub created_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Project DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// RelationsFormat enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RelationsFormat {
    Ids,
    Urls,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetWorkItemResult {
    pub id: i64,
    pub rev: Option<u64>,
    pub fields: WorkItemFields,
    pub comments: Option<Vec<CommentDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<RelationDto>>,
}

impl GetWorkItemResult {
    /// Convert relations to the requested format.
    pub fn with_relations_format(mut self, format: &RelationsFormat) -> Self {
        if let Some(ref mut rels) = self.relations {
            match format {
                RelationsFormat::Ids => {
                    for rel in rels.iter_mut() {
                        if let Some(id) = RelationDto::work_item_id_from_url(&rel.url) {
                            rel.url = id.to_string();
                        }
                    }
                }
                RelationsFormat::Urls => {}
            }
        }
        self
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateWorkItemResult {
    pub id: i64,
    pub url: String,
    pub fields: WorkItemFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<RelationDto>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ManageWorkItemLinkResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<RelationDto>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchWorkItemsResult {
    pub ids: Vec<i64>,
    pub total: usize,
    pub details: Option<Vec<GetWorkItemResult>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetQueryResult {
    pub ids: Vec<i64>,
    pub total: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusConfigDto {
    pub token_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ado_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ado_default_project: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusResult {
    pub status: String,
    pub project_count: usize,
    pub projects: Vec<ProjectDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<StatusConfigDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListProjectsResult {
    pub projects: Vec<ProjectDto>,
    pub total: usize,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct GetWorkitemCommentsParams {
    pub id: WorkItemId,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetWorkitemCommentsResult {
    pub id: i64,
    pub comments: Vec<CommentDto>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// AddComment
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct AddCommentParams {
    pub id: WorkItemId,
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentResult {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_id: Option<i64>,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_id_to_i64_int() {
        let id = WorkItemId::Int(42);
        assert_eq!(id.to_i64().unwrap(), 42);
    }

    #[test]
    fn work_item_id_to_i64_str_valid() {
        let id = WorkItemId::Str("99".to_string());
        assert_eq!(id.to_i64().unwrap(), 99);
    }

    #[test]
    fn work_item_id_to_i64_str_invalid() {
        let id = WorkItemId::Str("abc".to_string());
        let err = id.to_i64().unwrap_err();
        assert!(err.to_string().contains("invalid id"));
    }

    #[test]
    fn value_to_workitemfields_with_title() {
        let v = serde_json::json!({"System.Title": "My Task"});
        let wf = super::super::conversions::value_to_workitemfields(v);
        assert_eq!(wf.title.as_deref(), Some("My Task"));
    }

    #[test]
    fn value_to_workitemfields_with_extra_fields() {
        let v = serde_json::json!({"Custom.Field1": "val1", "Custom.Field2": 42});
        let wf = super::super::conversions::value_to_workitemfields(v);
        assert!(wf.title.is_none());
        assert!(wf.extra.is_some());
        let extra = wf.extra.unwrap();
        assert_eq!(extra.get("Custom.Field1").unwrap(), "val1");
    }

    #[test]
    fn value_to_workitemfields_with_raw_value() {
        let v = serde_json::json!("just a string");
        let wf = super::super::conversions::value_to_workitemfields(v);
        assert!(wf.title.is_none());
        assert!(wf.extra.is_some());
        let extra = wf.extra.unwrap();
        assert!(extra.contains_key("__raw"));
    }

    #[test]
    fn relation_dto_work_item_id_from_url_valid() {
        let url = "https://dev.azure.com/org/_apis/wit/workItems/123";
        assert_eq!(RelationDto::work_item_id_from_url(url), Some(123));
    }

    #[test]
    fn relation_dto_work_item_id_from_url_invalid() {
        let url = "https://example.com/no-id-here";
        assert_eq!(RelationDto::work_item_id_from_url(url), None);
    }

    #[test]
    fn get_work_item_result_with_relations_format_ids() {
        let result = GetWorkItemResult {
            id: 1,
            rev: Some(1),
            fields: WorkItemFields {
                title: None,
                description: None,
                extra: None,
            },
            comments: None,
            relations: Some(vec![RelationDto {
                url: "https://dev.azure.com/org/_apis/wit/workItems/42".to_string(),
                rel: "System.LinkTypes.Related".to_string(),
                attributes: None,
            }]),
        };
        let converted = result.with_relations_format(&RelationsFormat::Ids);
        assert_eq!(converted.relations.unwrap()[0].url, "42");
    }

    #[test]
    fn get_work_item_result_with_relations_format_urls() {
        let result = GetWorkItemResult {
            id: 1,
            rev: Some(1),
            fields: WorkItemFields {
                title: None,
                description: None,
                extra: None,
            },
            comments: None,
            relations: Some(vec![RelationDto {
                url: "https://dev.azure.com/org/_apis/wit/workItems/42".to_string(),
                rel: "System.LinkTypes.Related".to_string(),
                attributes: None,
            }]),
        };
        let converted = result.with_relations_format(&RelationsFormat::Urls);
        assert_eq!(
            converted.relations.unwrap()[0].url,
            "https://dev.azure.com/org/_apis/wit/workItems/42"
        );
    }

    #[test]
    fn get_work_item_result_with_relations_format_no_relations() {
        let result = GetWorkItemResult {
            id: 1,
            rev: Some(1),
            fields: WorkItemFields {
                title: None,
                description: None,
                extra: None,
            },
            comments: None,
            relations: None,
        };
        let converted = result.with_relations_format(&RelationsFormat::Ids);
        assert!(converted.relations.is_none());
    }

    #[test]
    fn work_item_fields_serde_roundtrip() {
        let wf = WorkItemFields {
            title: Some("T".into()),
            description: Some("D".into()),
            extra: None,
        };
        let json = serde_json::to_value(&wf).unwrap();
        assert_eq!(json.get("System.Title").unwrap(), "T");
        assert_eq!(json.get("System.Description").unwrap(), "D");
        let deserialized: WorkItemFields = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.title.as_deref(), Some("T"));
    }

    #[test]
    fn work_item_fields_serde_roundtrip_with_extra() {
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("Custom.Foo".to_string(), serde_json::json!("bar"));
        let wf = WorkItemFields {
            title: None,
            description: None,
            extra: Some(extra),
        };
        let json = serde_json::to_value(&wf).unwrap();
        assert_eq!(json.get("Custom.Foo").unwrap(), "bar");
    }
}
