use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::dto::{RelationDto, RelationsFormat, WorkItemId};

// ---------------------------------------------------------------------------
// GetStatusParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct GetStatusParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_secrets: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_projects: Option<bool>,
}

// ---------------------------------------------------------------------------
// ListProjectsParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct ListProjectsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// CreateWorkItemParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct CreateWorkItemParams {
    pub project: String,
    #[serde(rename = "type")]
    pub wi_type: String,
    pub fields: super::dto::WorkItemFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<RelationDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<super::dto::CommentDto>>,
}

// ---------------------------------------------------------------------------
// UpdateWorkItemParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[schemars(example = update_work_item_params_example())]
pub struct UpdateWorkItemParams {
    pub id: WorkItemId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    pub expected_rev: Option<u64>,
}

#[allow(dead_code)]
fn update_work_item_params_example() -> UpdateWorkItemParams {
    UpdateWorkItemParams {
        id: WorkItemId::Int(100),
        patch: Some(serde_json::json!([
            {
                "op": "add",
                "path": "/relations/-",
                "value": {
                    "rel": "System.LinkTypes.Related",
                    "url": "https://dev.azure.com/yourorg/_apis/wit/workItems/200"
                }
            }
        ])),
        tags: None,
        expected_rev: None,
    }
}

// ---------------------------------------------------------------------------
// ManageWorkItemLinkParams
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, JsonSchema, Clone)]
#[schemars(example = manage_work_item_link_params_example())]
pub struct ManageWorkItemLinkParams {
    pub action: String,
    pub source_id: WorkItemId,
    pub target: WorkItemId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
}

impl<'de> serde::Deserialize<'de> for ManageWorkItemLinkParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(deserializer).map_err(serde::de::Error::custom)?;
        let obj = v.as_object().ok_or_else(|| {
            serde::de::Error::custom("expected object for ManageWorkItemLinkParams")
        })?;

        if obj.contains_key("fields") && !obj.contains_key("attributes") {
            return Err(serde::de::Error::custom(
                "found top-level 'fields' in manage_work_item_link; 'fields' are used by 'update_work_item'. To add a comment to the relation, use 'comment' (string) or 'attributes' with { 'comment': '...' }. Example: {\"action\":\"add\", \"source_id\": 100, \"target\": 200, \"link_type\": \"System.LinkTypes.Related\", \"comment\": \"Linking related work item\"}",
            ));
        }

        let mut v = v.clone();
        if obj.contains_key("comment")
            && !obj.contains_key("attributes")
            && let Some(comment_val) = obj.get("comment")
        {
            let attrs = serde_json::json!({"comment": comment_val.clone()});
            if let Some(map) = v.as_object_mut() {
                map.insert("attributes".to_string(), attrs);
            }
        }

        let has_action = v.as_object().unwrap().contains_key("action");
        let has_source = v.as_object().unwrap().contains_key("source_id");
        let has_target = v.as_object().unwrap().contains_key("target");

        if has_action
            && has_source
            && has_target
            && (obj.contains_key("relationType")
                || obj.contains_key("relationtype")
                || obj.contains_key("linkType"))
            && !obj.contains_key("link_type")
        {
            return Err(serde::de::Error::custom(
                "use `link_type` (not `relationType` or `linkType`). Example: {\"action\":\"add\", \"source_id\": 100, \"target\": 200, \"link_type\": \"System.LinkTypes.Related\"}",
            ));
        }

        if !has_action || !has_source || !has_target {
            let mut hints: Vec<String> = Vec::new();
            if obj.contains_key("relationType")
                || obj.contains_key("relationtype")
                || obj.contains_key("linkType")
            {
                hints.push("use `link_type` (not `relationType` or `linkType`)".to_string());
            }
            if obj.contains_key("sourceWorkItemId") || obj.contains_key("sourceWorkItem") {
                hints.push("use `source_id` (not `sourceWorkItemId`)".to_string());
            }
            if obj.contains_key("targetWorkItemId") || obj.contains_key("targetWorkItem") {
                hints.push("use `target` (not `targetWorkItemId`)".to_string());
            }
            let mut msg = "missing required fields; expected keys: `action`, `source_id`, `target`"
                .to_string();
            if !hints.is_empty() {
                msg.push_str(". Common mistakes: ");
                msg.push_str(&hints.join(", "));
            }
            return Err(serde::de::Error::custom(msg));
        }

        #[derive(serde::Deserialize)]
        struct RawManageWorkItemLinkParams {
            action: String,
            source_id: WorkItemId,
            target: WorkItemId,
            #[serde(default)]
            link_type: Option<String>,
            #[serde(default)]
            attributes: Option<serde_json::Value>,
        }

        let raw: RawManageWorkItemLinkParams =
            serde_json::from_value(v).map_err(serde::de::Error::custom)?;
        Ok(ManageWorkItemLinkParams {
            action: raw.action,
            source_id: raw.source_id,
            target: raw.target,
            link_type: raw.link_type,
            attributes: raw.attributes,
        })
    }
}

#[allow(dead_code)]
fn manage_work_item_link_params_example() -> ManageWorkItemLinkParams {
    ManageWorkItemLinkParams {
        action: "add".to_string(),
        source_id: WorkItemId::Int(100),
        target: WorkItemId::Int(101),
        link_type: Some("System.LinkTypes.Hierarchy-Forward".to_string()),
        attributes: Some(serde_json::json!({"comment":"Linking as parent"})),
    }
}

// ---------------------------------------------------------------------------
// SearchWorkItemsParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct SearchWorkItemsParams {
    pub project: Option<String>,
    pub wiql: Option<String>,
    pub details: Option<bool>,
    pub limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_description: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations_format: Option<RelationsFormat>,
}

// ---------------------------------------------------------------------------
// GetQueryParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct GetQueryParams {
    pub url: Option<String>,
    pub project: Option<String>,
    pub query_id: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// GetWorkItemParams
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[schemars(example = get_work_item_params_example())]
pub struct GetWorkItemParams {
    pub id: Option<WorkItemId>,
    pub fields: Option<Vec<String>>,
    #[serde(rename = "$expand")]
    pub expand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_description: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations_format: Option<RelationsFormat>,
}

#[allow(dead_code)]
fn get_work_item_params_example() -> GetWorkItemParams {
    GetWorkItemParams {
        id: Some(WorkItemId::Int(100)),
        fields: None,
        expand: Some("relations".to_string()),
        include_comments: Some(false),
        include_description: Some(false),
        relations_format: Some(RelationsFormat::Ids),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manage_work_item_link_params_comment_shorthand() {
        let json = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "link_type": "System.LinkTypes.Related",
            "comment": "test comment"
        });
        let params: ManageWorkItemLinkParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "add");
        assert!(params.attributes.is_some());
        let attrs = params.attributes.unwrap();
        assert_eq!(attrs.get("comment").unwrap(), "test comment");
    }

    #[test]
    fn manage_work_item_link_params_with_attributes_directly() {
        let json = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "link_type": "System.LinkTypes.Related",
            "attributes": {"comment": "direct"}
        });
        let params: ManageWorkItemLinkParams = serde_json::from_value(json).unwrap();
        assert!(params.attributes.is_some());
    }

    #[test]
    fn manage_work_item_link_params_missing_fields() {
        let json = serde_json::json!({"action": "add"});
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
    }

    #[test]
    fn manage_work_item_link_params_fields_key_error() {
        let json = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "fields": {"System.Title": "test"}
        });
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fields"));
    }

    #[test]
    fn manage_work_item_link_params_relation_type_hint() {
        let json = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "target": 200,
            "relationType": "System.LinkTypes.Related"
        });
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("link_type"));
    }

    #[test]
    fn manage_work_item_link_params_source_work_item_id_hint() {
        let json = serde_json::json!({
            "action": "add",
            "sourceWorkItemId": 100,
            "target": 200
        });
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("source_id"));
    }

    #[test]
    fn manage_work_item_link_params_target_work_item_id_hint() {
        let json = serde_json::json!({
            "action": "add",
            "source_id": 100,
            "targetWorkItemId": 200
        });
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("target"));
    }

    #[test]
    fn manage_work_item_link_params_not_object() {
        let json = serde_json::json!("just a string");
        let result = serde_json::from_value::<ManageWorkItemLinkParams>(json);
        assert!(result.is_err());
    }
}
