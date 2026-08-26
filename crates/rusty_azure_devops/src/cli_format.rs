use crate::domain::*;

pub fn parse_work_item_id(s: &str) -> WorkItemId {
    s.parse::<i64>()
        .map(WorkItemId::Int)
        .unwrap_or_else(|_| WorkItemId::Str(s.to_string()))
}

fn format_json<T: serde::Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    );
}

pub fn format_status(result: &StatusResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!("Status: {}", result.status);
    println!("Projects: {}", result.project_count);
    if let Some(ref proj) = result.default_project {
        println!("Default project: {}", proj);
    }
    if !result.projects.is_empty() {
        println!("Projects list:");
        for p in &result.projects {
            println!("  - {} ({})", p.name, p.id);
        }
    }
    if let Some(ref cfg) = result.config {
        println!("Config:");
        println!("  token_present: {}", cfg.token_present);
        if let Some(ref url) = cfg.ado_base_url {
            println!("  base_url: {}", url);
        }
        if let Some(ref proj) = cfg.ado_default_project {
            println!("  default_project: {}", proj);
        }
    }
}

pub fn format_list_projects(result: &ListProjectsResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!("Total: {}", result.total);
    for p in &result.projects {
        println!("  {} ({})", p.name, p.id);
    }
}

pub fn format_work_item(result: &GetWorkItemResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!("Work Item #{}", result.id);
    if let Some(rev) = result.rev {
        println!("  Rev: {}", rev);
    }
    if let Some(ref title) = result.fields.title {
        println!("  Title: {}", title);
    }
    if let Some(ref desc) = result.fields.description {
        println!("  Description: {}", desc);
    }
    if let Some(ref extra) = result.fields.extra {
        for (k, v) in extra {
            println!("  {}: {}", k, v);
        }
    }
    if let Some(ref relations) = result.relations
        && !relations.is_empty()
    {
        println!("  Relations:");
        for r in relations {
            println!("    {} -> {} ({})", r.rel, r.url, r.rel);
        }
    }
    if let Some(ref comments) = result.comments
        && !comments.is_empty()
    {
        println!("  Comments:");
        for c in comments {
            println!(
                "    [{}] {}: {}",
                c.created_at.as_deref().unwrap_or("?"),
                c.author.as_deref().unwrap_or("?"),
                c.text
            );
        }
    }
}

pub fn format_create_result(result: &CreateWorkItemResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!("Created work item #{}", result.id);
    println!("URL: {}", result.url);
    if let Some(ref title) = result.fields.title {
        println!("Title: {}", title);
    }
}

pub fn format_link_result(result: &ManageWorkItemLinkResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    if result.success {
        println!("Success");
    } else if let Some(ref msg) = result.message {
        println!("Failed: {}", msg);
    }
    if let Some(ref relations) = result.relations {
        println!("Relations:");
        for r in relations {
            println!("  {} -> {}", r.rel, r.url);
        }
    }
}

pub fn format_search(result: &SearchWorkItemsResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!("Found {} items (total: {})", result.ids.len(), result.total);
    if result.ids.is_empty() {
        return;
    }
    if let Some(ref details) = result.details {
        for wi in details {
            print!("  #{}", wi.id);
            if let Some(ref title) = wi.fields.title {
                print!(" - {}", title);
            }
            println!();
        }
    } else {
        for id in &result.ids {
            println!("  #{}", id);
        }
    }
}

pub fn format_comments(result: &GetWorkitemCommentsResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    println!(
        "Comments for work item #{} ({} total):",
        result.id, result.total
    );
    for c in &result.comments {
        println!(
            "  [{}] {}:\n    {}",
            c.created_at.as_deref().unwrap_or("?"),
            c.author.as_deref().unwrap_or("anonymous"),
            c.text
        );
    }
}

pub fn format_add_comment_result(result: &AddCommentResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    match result.comment_id {
        Some(cid) => println!("Comment {} added to work item #{}", cid, result.id),
        None => println!(
            "Comment added to work item #{} (via System.History fallback)",
            result.id
        ),
    }
}

pub fn format_query_results(result: &GetQueryResult, json: bool) {
    if json {
        format_json(result);
        return;
    }
    if result.ids.is_empty() {
        println!("No work item IDs found (total: {}).", result.total);
    } else {
        println!(
            "Found {} work item IDs (total: {}): {:?}",
            result.ids.len(),
            result.total,
            result.ids
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_work_item_id_integer() {
        let id = parse_work_item_id("42");
        assert_eq!(id.to_i64().unwrap(), 42);
    }

    #[test]
    fn parse_work_item_id_string() {
        let id = parse_work_item_id("abc");
        assert!(matches!(id, WorkItemId::Str(_)));
    }

    #[test]
    fn format_work_item_json_output() {
        let result = GetWorkItemResult {
            id: 1,
            rev: Some(1),
            fields: WorkItemFields {
                title: Some("Test".into()),
                description: None,
                extra: None,
            },
            comments: None,
            relations: None,
        };
        format_work_item(&result, true);
    }

    #[test]
    fn format_create_result_json_output() {
        let result = CreateWorkItemResult {
            id: 1,
            url: "workitems/1".into(),
            fields: WorkItemFields {
                title: Some("Test".into()),
                description: None,
                extra: None,
            },
            relations: None,
        };
        format_create_result(&result, true);
    }

    #[test]
    fn format_search_empty() {
        let result = SearchWorkItemsResult {
            ids: vec![],
            total: 0,
            details: None,
        };
        format_search(&result, true);
    }

    #[test]
    fn format_status_json_output() {
        let result = StatusResult {
            status: "ok".into(),
            project_count: 2,
            projects: vec![ProjectDto {
                id: "1".into(),
                name: "P1".into(),
            }],
            default_project: Some("P1".into()),
            config: None,
        };
        format_status(&result, true);
    }

    #[test]
    fn format_status_human_readable() {
        let result = StatusResult {
            status: "ok".into(),
            project_count: 1,
            projects: vec![],
            default_project: None,
            config: None,
        };
        format_status(&result, false);
    }

    #[test]
    fn format_status_human_with_config() {
        let result = StatusResult {
            status: "ok".into(),
            project_count: 1,
            projects: vec![ProjectDto {
                id: "1".into(),
                name: "Demo".into(),
            }],
            default_project: Some("Demo".into()),
            config: Some(StatusConfigDto {
                token_present: true,
                ado_base_url: Some("https://tfs.example.com".into()),
                ado_default_project: Some("Demo".into()),
            }),
        };
        format_status(&result, false);
        format_status(&result, true);
    }

    #[test]
    fn format_work_item_human_readable() {
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("System.Tags".into(), serde_json::json!("urgent"));
        extra.insert("Custom.Field".into(), serde_json::json!("value"));
        let result = GetWorkItemResult {
            id: 42,
            rev: Some(2),
            fields: WorkItemFields {
                title: Some("Test".into()),
                description: Some("desc".into()),
                extra: Some(extra),
            },
            comments: Some(vec![CommentDto {
                id: Some(1),
                author: Some("alice".into()),
                text: "looks good".into(),
                created_at: Some("2024-01-01".into()),
            }]),
            relations: Some(vec![RelationDto {
                url: "42".into(),
                rel: "System.LinkTypes.Related".into(),
                attributes: Some(RelationAttributes {
                    comment: Some("because".into()),
                }),
            }]),
        };
        format_work_item(&result, true);
        format_work_item(&result, false);
    }

    #[test]
    fn format_link_result_human_readable() {
        let result = ManageWorkItemLinkResult {
            success: true,
            message: None,
            relations: Some(vec![RelationDto {
                url: "100".into(),
                rel: "System.LinkTypes.Related".into(),
                attributes: None,
            }]),
        };
        format_link_result(&result, true);
        format_link_result(&result, false);

        let failed = ManageWorkItemLinkResult {
            success: false,
            message: Some("not found".into()),
            relations: None,
        };
        format_link_result(&failed, true);
        format_link_result(&failed, false);
    }

    #[test]
    fn format_create_result_human_readable() {
        let result = CreateWorkItemResult {
            id: 1,
            url: "workitems/1".into(),
            fields: WorkItemFields {
                title: Some("Created".into()),
                description: None,
                extra: None,
            },
            relations: None,
        };
        format_create_result(&result, false);
    }

    #[test]
    fn format_comments_human_readable() {
        let result = GetWorkitemCommentsResult {
            id: 42,
            comments: vec![CommentDto {
                id: Some(1),
                author: Some("bob".into()),
                text: "first".into(),
                created_at: Some("2024-01-01".into()),
            }],
            total: 1,
        };
        format_comments(&result, true);
        format_comments(&result, false);
    }

    #[test]
    fn format_search_with_details_human() {
        let result = SearchWorkItemsResult {
            ids: vec![1, 2],
            total: 2,
            details: Some(vec![
                GetWorkItemResult {
                    id: 1,
                    rev: None,
                    fields: WorkItemFields {
                        title: Some("A".into()),
                        description: None,
                        extra: None,
                    },
                    comments: None,
                    relations: None,
                },
                GetWorkItemResult {
                    id: 2,
                    rev: None,
                    fields: WorkItemFields {
                        title: Some("B".into()),
                        description: None,
                        extra: None,
                    },
                    comments: None,
                    relations: None,
                },
            ]),
        };
        format_search(&result, true);
        format_search(&result, false);
    }

    #[test]
    fn format_search_only_ids_human() {
        let result = SearchWorkItemsResult {
            ids: vec![1, 2],
            total: 2,
            details: None,
        };
        format_search(&result, false);
    }

    #[test]
    fn format_add_comment_result_with_comment_id() {
        let result = AddCommentResult {
            id: 123,
            comment_id: Some(456),
            text: "test".into(),
        };
        format_add_comment_result(&result, false);
    }

    #[test]
    fn format_add_comment_result_fallback() {
        let result = AddCommentResult {
            id: 123,
            comment_id: None,
            text: "test".into(),
        };
        format_add_comment_result(&result, false);
    }

    #[test]
    fn format_add_comment_result_json() {
        let result = AddCommentResult {
            id: 123,
            comment_id: Some(456),
            text: "test".into(),
        };
        format_add_comment_result(&result, true);
    }

    #[test]
    fn format_query_results_with_ids() {
        let result = GetQueryResult {
            ids: vec![1, 2, 3],
            total: 3,
        };
        format_query_results(&result, false);
    }

    #[test]
    fn format_query_results_empty() {
        let result = GetQueryResult {
            ids: vec![],
            total: 0,
        };
        format_query_results(&result, false);
    }

    #[test]
    fn format_query_results_json() {
        let result = GetQueryResult {
            ids: vec![1, 2],
            total: 10,
        };
        format_query_results(&result, true);
    }
}
