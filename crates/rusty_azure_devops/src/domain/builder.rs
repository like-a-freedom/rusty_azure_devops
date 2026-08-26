use std::collections::BTreeMap;

use azure_devops_client::AzureError;
use azure_devops_client::HttpError;

use super::conversions::{convert_description_value, markdown_to_html};
use super::dto::{CommentDto, RelationDto, WorkItemFields};
use super::params::CreateWorkItemParams;

/// Build [`CreateWorkItemParams`] from raw CLI inputs.
///
/// Handles Markdown→HTML conversion for `System.Description` and wraps
/// empty collections into `Option` for the params struct. The MCP path
/// deserializes JSON directly via serde — this builder is for the CLI path.
pub fn build_create_work_item_params(
    project: String,
    wi_type: String,
    title: String,
    description: Option<String>,
    field_args: BTreeMap<String, serde_json::Value>,
    relations: Vec<RelationDto>,
    comments: Vec<String>,
) -> Result<CreateWorkItemParams, AzureError> {
    if title.trim().is_empty() {
        return Err(AzureError::Http(HttpError::decode(
            "missing required field System.Title",
        )));
    }

    let mut extra = field_args;
    // Convert System.Description from Markdown to HTML
    if let Some(desc_val) = extra.remove("System.Description") {
        extra.insert(
            "System.Description".to_string(),
            convert_description_value(&desc_val),
        );
    }

    let fields = WorkItemFields {
        title: Some(title),
        description: description.map(|d| markdown_to_html(&d)),
        extra: if extra.is_empty() { None } else { Some(extra) },
    };

    let rels = if relations.is_empty() {
        None
    } else {
        Some(relations)
    };

    let comment_dtos = if comments.is_empty() {
        None
    } else {
        Some(
            comments
                .into_iter()
                .map(|text| CommentDto {
                    id: None,
                    author: None,
                    text,
                    created_at: None,
                })
                .collect(),
        )
    };

    Ok(CreateWorkItemParams {
        project,
        wi_type,
        fields,
        relations: rels,
        comments: comment_dtos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_params_minimal() {
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "My Task".into(),
            None,
            BTreeMap::new(),
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(params.project, "Demo");
        assert_eq!(params.wi_type, "Task");
        assert_eq!(params.fields.title.as_deref(), Some("My Task"));
        assert!(params.fields.description.is_none());
        assert!(params.fields.extra.is_none());
        assert!(params.relations.is_none());
        assert!(params.comments.is_none());
    }

    #[test]
    fn build_params_with_description_markdown_to_html() {
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "T".into(),
            Some("**bold** text".into()),
            BTreeMap::new(),
            vec![],
            vec![],
        )
        .unwrap();
        let desc = params.fields.description.unwrap();
        assert!(desc.contains("<strong>bold</strong>"), "got: {}", desc);
    }

    #[test]
    fn build_params_with_field_description_markdown_to_html() {
        let mut extra = BTreeMap::new();
        extra.insert(
            "System.Description".to_string(),
            serde_json::Value::String("*italic*".to_string()),
        );
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "T".into(),
            None,
            extra,
            vec![],
            vec![],
        )
        .unwrap();
        let desc = params
            .fields
            .extra
            .unwrap()
            .get("System.Description")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        assert!(desc.contains("<em>italic</em>"), "got: {}", desc);
    }

    #[test]
    fn build_params_with_custom_fields() {
        let mut extra = BTreeMap::new();
        extra.insert(
            "System.Tags".to_string(),
            serde_json::Value::String("urgent;ops".into()),
        );
        extra.insert("Custom.Estimate".to_string(), serde_json::json!(5));
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "T".into(),
            None,
            extra,
            vec![],
            vec![],
        )
        .unwrap();
        let extra = params.fields.extra.unwrap();
        assert_eq!(
            extra.get("System.Tags").unwrap().as_str().unwrap(),
            "urgent;ops"
        );
        assert_eq!(extra.get("Custom.Estimate").unwrap(), 5);
    }

    #[test]
    fn build_params_with_relations() {
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "T".into(),
            None,
            BTreeMap::new(),
            vec![RelationDto {
                url: "https://example.com/workItems/456".into(),
                rel: "System.LinkTypes.Related".into(),
                attributes: None,
            }],
            vec![],
        )
        .unwrap();
        let rels = params.relations.unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].rel, "System.LinkTypes.Related");
    }

    #[test]
    fn build_params_with_comments() {
        let params = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "T".into(),
            None,
            BTreeMap::new(),
            vec![],
            vec!["first comment".into(), "second comment".into()],
        )
        .unwrap();
        let comments = params.comments.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "first comment");
        assert_eq!(comments[1].text, "second comment");
        assert!(comments[0].id.is_none());
        assert!(comments[0].author.is_none());
    }

    #[test]
    fn build_params_empty_title_rejected() {
        let result = build_create_work_item_params(
            "Demo".into(),
            "Task".into(),
            "  ".into(),
            None,
            BTreeMap::new(),
            vec![],
            vec![],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("System.Title"));
    }
}
