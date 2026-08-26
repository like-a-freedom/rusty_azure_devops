use azure_devops_client::{Comment, Project};

use super::dto::{CommentDto, ProjectDto, RelationDto, WorkItemFields};

/// Convert HTML to Markdown for token-efficient text representation.
pub fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_string())
}

/// Convert Markdown to HTML for writing back to Azure DevOps `System.Description`.
pub fn markdown_to_html(md: &str) -> String {
    let parser = pulldown_cmark::Parser::new(md);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
}

/// Apply Markdown→HTML conversion to a `System.Description` JSON value.
/// Non-string values pass through unchanged.
pub fn convert_description_value(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::String(s) => serde_json::Value::String(markdown_to_html(s)),
        other => other.clone(),
    }
}

/// Convert arbitrary Value -> WorkItemFields used by DTOs when SDK returns dynamic fields.
pub fn value_to_workitemfields(v: serde_json::Value) -> WorkItemFields {
    if let Ok(wf) = serde_json::from_value::<WorkItemFields>(v.clone()) {
        return wf;
    }
    let extra = v.as_object().map(|m| m.clone().into_iter().collect());
    if extra.is_some() {
        WorkItemFields {
            title: None,
            description: None,
            extra,
        }
    } else {
        let mut map = std::collections::BTreeMap::new();
        map.insert("__raw".to_string(), v);
        WorkItemFields {
            title: None,
            description: None,
            extra: Some(map),
        }
    }
}

/// Build JSON Patch ops from `WorkItemFields`. Each non-None field becomes
/// `{op, path: "/fields/<key>", value}`. The caller passes the op name
/// (`"add"` for create, `"replace"` for update).
pub fn fields_to_patch_ops(fields: &WorkItemFields, op: &str) -> Vec<serde_json::Value> {
    let mut ops = Vec::new();
    if let Some(ref title) = fields.title {
        ops.push(serde_json::json!({"op": op, "path": "/fields/System.Title", "value": title}));
    }
    if let Some(ref description) = fields.description {
        ops.push(serde_json::json!({"op": op, "path": "/fields/System.Description", "value": description}));
    }
    if let Some(ref extra) = fields.extra {
        for (k, v) in extra {
            if v.is_null() {
                continue;
            }
            ops.push(serde_json::json!({"op": op, "path": format!("/fields/{}", k), "value": v}));
        }
    }
    ops
}

/// Build a single JSON Patch op to add/replace a relation. The caller passes
/// the op name (`"add"` for create, `"replace"` for update).
pub fn relation_to_patch(r: &RelationDto, op: &str) -> serde_json::Value {
    let mut value = serde_json::json!({"url": r.url, "rel": r.rel});
    if let Some(ref attrs) = r.attributes
        && let Ok(a) = serde_json::to_value(attrs)
        && let Some(obj) = value.as_object_mut()
    {
        obj.insert("attributes".to_string(), a);
    }
    serde_json::json!({"op": op, "path": "/relations/-", "value": value})
}

impl From<Project> for ProjectDto {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
        }
    }
}

impl From<Comment> for CommentDto {
    fn from(c: Comment) -> Self {
        Self {
            id: c.id,
            author: c.author,
            text: c.text,
            created_at: c.created_at,
        }
    }
}

impl From<azure_devops_client::Relation> for RelationDto {
    fn from(r: azure_devops_client::Relation) -> Self {
        let attrs = r.attributes.and_then(|v| {
            if let Some(obj) = v.as_object() {
                let comment = obj
                    .get("comment")
                    .and_then(|c| c.as_str().map(|s| s.to_string()));
                Some(super::dto::RelationAttributes { comment })
            } else {
                None
            }
        });
        Self {
            url: r.url,
            rel: r
                .rel
                .unwrap_or_else(|| "System.LinkTypes.Related".to_string()),
            attributes: attrs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_markdown_converts_basic_html() {
        let result = html_to_markdown("<p>Hello <b>world</b></p>");
        assert!(result.contains("Hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn html_to_markdown_returns_original_on_invalid() {
        let result = html_to_markdown("");
        assert_eq!(result, "");
    }

    #[test]
    fn markdown_to_html_converts_basic_markdown() {
        let result = markdown_to_html("# Hello **world**");
        assert!(result.contains("<h1>"));
        assert!(result.contains("<strong>world</strong>"));
    }

    #[test]
    fn markdown_to_html_converts_list() {
        let result = markdown_to_html("- item 1\n- item 2");
        assert!(result.contains("<ul>"));
        assert!(result.contains("<li>item 1</li>"));
        assert!(result.contains("<li>item 2</li>"));
    }

    #[test]
    fn markdown_to_html_converts_paragraph() {
        let result = markdown_to_html("simple text");
        assert_eq!(result, "<p>simple text</p>\n");
    }

    #[test]
    fn markdown_to_html_empty_string() {
        let result = markdown_to_html("");
        assert_eq!(result, "");
    }

    #[test]
    fn markdown_to_html_roundtrip_identity() {
        let original = "<p>Hello <strong>world</strong></p>";
        let md = html_to_markdown(original);
        let html = markdown_to_html(&md);
        assert_eq!(html, "<p>Hello <strong>world</strong></p>\n");
    }

    #[test]
    fn relation_dto_from_client_relation() {
        let r = azure_devops_client::Relation {
            rel: Some("System.LinkTypes.Related".to_string()),
            url: "https://example.com/workItems/42".to_string(),
            attributes: Some(serde_json::json!({"comment": "test"})),
        };
        let dto: RelationDto = r.into();
        assert_eq!(dto.rel, "System.LinkTypes.Related");
        assert_eq!(dto.url, "https://example.com/workItems/42");
        assert!(dto.attributes.is_some());
        assert_eq!(dto.attributes.unwrap().comment.as_deref(), Some("test"));
    }

    #[test]
    fn relation_dto_from_client_relation_no_attributes() {
        let r = azure_devops_client::Relation {
            rel: None,
            url: "https://example.com/workItems/1".to_string(),
            attributes: None,
        };
        let dto: RelationDto = r.into();
        assert_eq!(dto.rel, "System.LinkTypes.Related");
        assert!(dto.attributes.is_none());
    }

    #[test]
    fn relation_dto_from_client_relation_non_object_attributes() {
        let r = azure_devops_client::Relation {
            rel: Some("Hyperlink".to_string()),
            url: "https://example.com".to_string(),
            attributes: Some(serde_json::json!("just a string")),
        };
        let dto: RelationDto = r.into();
        assert!(dto.attributes.is_none());
    }

    #[test]
    fn project_dto_from_client_project() {
        let p = Project {
            id: "10".into(),
            name: "P10".into(),
        };
        let dto: ProjectDto = p.into();
        assert_eq!(dto.id, "10");
        assert_eq!(dto.name, "P10");
    }

    #[test]
    fn comment_dto_from_client_comment() {
        let c = Comment {
            id: Some(5),
            author: Some("alice".into()),
            text: "hello".into(),
            created_at: Some("2024-01-01".into()),
        };
        let dto: CommentDto = c.into();
        assert_eq!(dto.id, Some(5));
        assert_eq!(dto.author.as_deref(), Some("alice"));
        assert_eq!(dto.text, "hello");
    }
}
