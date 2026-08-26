use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use rusty_azure_devops::{
    build_create_work_item_params, config::Config, create_modules, domain::*,
    format_add_comment_result, format_comments, format_create_result, format_link_result,
    format_list_projects, format_query_results, format_search, format_status, format_work_item,
    parse_work_item_id,
};

fn parse_key_value(s: &str) -> Result<(String, Value), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
        .ok_or_else(|| {
            format!(
                "invalid --field format: '{}'. Expected KEY=VALUE (e.g. --field 'System.Tags=urgent')",
                s
            )
        })
}

fn parse_relation(s: &str) -> Result<RelationDto, String> {
    let mut url = None;
    let mut rel = None;
    let mut comment = None;
    for part in s.split(',') {
        let (k, v) = part.split_once('=').ok_or_else(|| {
            format!(
                "invalid --relation format: '{}'. Expected key=value pairs (e.g. --relation 'url=...,rel=System.LinkTypes.Related')",
                s
            )
        })?;
        match k.trim() {
            "url" => url = Some(v.trim().to_string()),
            "rel" => rel = Some(v.trim().to_string()),
            "comment" => comment = Some(v.trim().to_string()),
            unknown => {
                return Err(format!(
                    "unknown key '{}' in --relation. Expected: url, rel, comment",
                    unknown
                ));
            }
        }
    }
    let url = url.ok_or_else(|| format!("--relation missing required key 'url': {}", s))?;
    let rel = rel.ok_or_else(|| format!("--relation missing required key 'rel': {}", s))?;
    Ok(RelationDto {
        url,
        rel,
        attributes: comment.map(|c| RelationAttributes { comment: Some(c) }),
    })
}

/// Azure DevOps CLI client. Agent-first tool for work item, project, and
/// comment operations. All commands accept --json for machine-readable output.
/// Credentials: --token flag, ADO_TOKEN env var, or config.yaml. See status.
#[derive(Parser)]
#[command(name = "ado-cli", version, about = "Azure DevOps CLI client")]
struct Cli {
    /// Personal Access Token. Overrides ADO_TOKEN env var and config.yaml.
    /// Ali: set ADO_TOKEN env var to avoid passing --token every time.
    #[arg(long, global = true)]
    token: Option<String>,

    /// Base URL of Azure DevOps Server (e.g. https://tfs.example.com/tfs/DefaultCollection).
    /// Overrides ADO_BASE_URL env var and config.yaml.
    #[arg(long, global = true)]
    base_url: Option<String>,

    /// Output as JSON instead of human-readable text.
    #[arg(long, global = true, default_value_t = false)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[doc = "Get server status. Shows connection health, project count, and configured default project.\nAlso shows config with --show-secrets (token presence, base URL, default project)."]
    Status {
        /// Show non-sensitive config: token presence, base URL, default project.
        /// These are not secrets — safe to read.
        #[arg(long)]
        show_secrets: bool,

        /// List all project names (default: only project count).
        #[arg(long)]
        include_projects: bool,
    },

    #[doc = "List Azure DevOps team projects. Default returns all projects.\nUse --offset and --limit to page through large lists."]
    ListProjects {
        /// Number of projects to skip (pagination). Use with --limit.
        #[arg(long)]
        offset: Option<usize>,

        /// Maximum number of projects to return.
        #[arg(long)]
        limit: Option<usize>,
    },

    #[doc = "Get a work item by ID. Returns a curated set of fields by default:\nid, rev, type, state, title, assignee, dates, tags, team project.\nUse --fields to select specific fields, or --fields '*' for all.\nUse --description to include System.Description (HTML→Markdown).\nThe returned Markdown can be fed back via --description or\n--field 'System.Description=...' on create/update (auto-converted back to HTML).\nUse --comments to fetch the discussion thread.\nUse --expand 'relations' or --expand 'All' for Azure API expand.\n\nOn-prem TFS: --expand drops --fields (server limitation). Use --comments\ninstead of --expand 'Comments' (not supported on-prem)."]
    GetWorkItem {
        /// Work item ID (integer).
        id: String,

        /// Comma-separated field names (e.g. 'System.Title,System.State').
        /// Default: id,rev,type,state,title,assignee,dates,tags,team project.
        /// Use '*' (with quotes) to return ALL fields.
        #[arg(long, value_delimiter = ',')]
        fields: Option<Vec<String>>,

        /// Azure API $expand value: 'relations', 'Comments', or 'All'.
        #[arg(long)]
        expand: Option<String>,

        /// Fetch discussion thread (comments).
        #[arg(long)]
        comments: bool,

        /// Include System.Description as Markdown (HTML converted to plain text).
        #[arg(long)]
        description: bool,

        /// Relation format: 'ids' (numeric work item IDs) or 'urls' (full API URLs).
        #[arg(long, value_parser = ["ids", "urls"])]
        format: Option<String>,
    },

    #[doc = "Create a work item in a project. Requires --project, --type, and --title.\nUse --field to set any work item field by reference name.\nUse --field multiple times to set multiple fields.\n\nCommon field reference names:\n  System.Title                       — title (also via --title)\n  System.Description                  — description (also via --description)\n  System.AssignedTo                   — assignee (e.g. 'user@example.com')\n  System.AreaPath                     — area path (e.g. 'Project\\\\Team')\n  System.IterationPath                — iteration path\n  System.Tags                         — semicolon-separated tags (e.g. 'urgent;ops')\n  Microsoft.VSTS.CMMI.RequirementType — 'Functional', 'Business Requirement', etc.\n  Custom.*                            — custom fields\n\nExamples:\n  ado-cli create-work-item --project MyProject --type Requirement --title 'Demo'\n  ado-cli create-work-item --project MyProject --type Task --title 'Fix' \\\n    --field 'System.Tags=urgent' \\\n    --field 'System.AreaPath=MyProject\\\\Team'"]
    CreateWorkItem {
        /// Project name (e.g. 'MyProject'). List projects first.
        #[arg(long)]
        project: String,

        /// Work item type (e.g. Task, Bug, Requirement, User Story, Epic, Feature, Issue).
        #[arg(long)]
        r#type: String,

        /// Title (System.Title). Required.
        #[arg(long)]
        title: String,

        /// Description text (System.Description). Markdown is auto-converted to HTML.
        #[arg(long)]
        description: Option<String>,

        /// Set a field by reference name: KEY=VALUE.
        /// Repeatable: --field 'System.Tags=urgent' --field 'System.AreaPath=Project\Team'.
        #[arg(long = "field", value_name = "KEY=VALUE", value_parser = parse_key_value)]
        field_args: Vec<(String, Value)>,

        /// Add a relation at creation time: key=value pairs (url, rel, comment).
        /// Repeatable: --relation 'url=https://...,rel=System.LinkTypes.Related,comment=linked'.
        /// Note: URLs must not contain commas (split on commas for key=value parsing).
        #[arg(long = "relation", value_name = "key=value,...", value_parser = parse_relation)]
        relations: Vec<RelationDto>,

        /// Add a discussion comment at creation time.
        /// Repeatable: --comment 'text here'.
        #[arg(long = "comment")]
        comments: Vec<String>,
    },

    #[doc = "Update a work item by ID. Use --field KEY=VALUE for simple field updates\n(repeatable), or --patch for arbitrary JSON Patch operations.\nUse --expected-rev to prevent overwriting concurrent edits.\n\nExamples:\n  ado-cli update-work-item 123 --field 'System.Tags=urgent'\n  ado-cli update-work-item 123 --field 'System.State=Resolved'\n  ado-cli update-work-item 123 --patch '[{\"op\":\"add\",\"path\":\"/relations/-\",\"value\":...}]'\n\nTip: --field generates 'replace' ops (works on existing fields).\n     For adding tags, use --field 'System.Tags=newtag' (overwrites existing tags)."]
    UpdateWorkItem {
        /// Work item ID (integer).
        id: String,

        /// JSON Patch array for complex updates (alternative to --field).
        /// Use for: adding relations, nested operations, or when --field is not enough.
        #[arg(long)]
        patch: Option<String>,

        /// Set a field value (generates 'replace' op): KEY=VALUE.
        /// Repeatable. Combined with --patch (field ops run first, patch can override).
        #[arg(long = "field", value_name = "KEY=VALUE", value_parser = parse_key_value)]
        field_args: Vec<(String, Value)>,

        /// Expected current revision for optimistic concurrency.
        /// If the work item was modified since you read it, the update fails.
        #[arg(long)]
        expected_rev: Option<u64>,
    },

    #[doc = "Manage work item links (relations).\nActions: 'add' (create a link) or 'remove' (delete a link from source to target).\nLink types: System.LinkTypes.Related, Hierarchy-Forward, Dependency-Forward,\nMicrosoft.VSTS.Common.Implementation, Hyperlink, etc.\n\nExamples:\n  ado-cli manage-link add 123 456 --link-type System.LinkTypes.Related\n  ado-cli manage-link add 123 456 --link-type Hierarchy-Forward --comment 'parent'\n  ado-cli manage-link remove 123 456"]
    ManageLink {
        /// Action: 'add' or 'remove'.
        action: String,

        /// Source work item ID (integer).
        source_id: String,

        /// Target work item ID (integer) or URL (for Hyperlink or Remote links).
        target: String,

        /// Relation type name (e.g. 'System.LinkTypes.Related', 'Hierarchy-Forward', 'Hyperlink').
        /// Defaults to 'Related' when omitted.
        #[arg(long)]
        link_type: Option<String>,

        /// Comment text for the relation.
        #[arg(long)]
        comment: Option<String>,
    },

    #[doc = "Search work items using WIQL (Work Item Query Language).\nReturns IDs by default; add --details for full records.\n--project defaults to configured default project if omitted.\n\nExamples:\n  ado-cli search\n  ado-cli search --project MyProject --details\n  ado-cli search --wiql \"SELECT [Id] FROM WorkItems WHERE [State] = 'Active'\" --details\n  ado-cli search --wiql \"SELECT [Id] FROM WorkItems WHERE [System.Title] CONTAINS 'foo' AND [System.TeamProject] = 'MyProject'\""]
    Search {
        /// Project name. Defaults to configured default project if omitted.
        project: Option<String>,

        /// WIQL query string. Example: "SELECT [Id] FROM WorkItems WHERE [State] = 'Active'"
        /// When omitted, returns work items assigned to the caller (if supported).
        #[arg(long)]
        wiql: Option<String>,

        /// Return full work item records (id, type, state, title, assignee) instead of just IDs.
        /// Combine with --description to include System.Description.
        #[arg(long)]
        details: bool,

        /// Maximum number of results to return.
        #[arg(long, default_value_t = 100)]
        limit: usize,

        /// Include System.Description in details (HTML converted to Markdown).
        #[arg(long)]
        description: bool,

        /// Relation format in details: 'ids' (numeric work item IDs) or 'urls' (full API URLs).
        #[arg(long, value_parser = ["ids", "urls"])]
        format: Option<String>,
    },

    #[doc = "Get all comments (discussion thread) for a work item by ID.\nReturns comment text, author name, and creation date.\nRecommended on-prem: use 'comments' instead of --expand 'Comments'."]
    Comments {
        /// Work item ID (integer).
        id: String,
    },

    #[doc = "Add a discussion comment to a work item by ID.\nThe comment appears in the work item's discussion thread.\nOn-prem TFS: falls back to System.History field if the Comments API is unavailable.\n\nExamples:\n  ado-cli add-comment 123 'Build passed'\n  ado-cli add-comment 123 'Investigate flaky test in CI'"]
    AddComment {
        /// Work item ID (integer).
        id: String,

        /// Comment text to post.
        text: String,
    },

    #[doc = "Execute a saved Azure DevOps query by URL or (project, query_id).\nReturns work item IDs. Supports query URLs from both on-prem and cloud.\n\nExamples:\n  ado-cli get-query-results --url 'https://host/Project/_queries/query/GUID'\n  ado-cli get-query-results --project MyProject --query-id '1c3ed7b3-...'\n  ado-cli get-query-results --url '...' --offset 200 --limit 100"]
    GetQueryResults {
        /// Full query URL (e.g. https://host/Project/_queries/query/GUID).
        #[arg(long)]
        url: Option<String>,

        /// Project name (required when using --query-id).
        #[arg(long)]
        project: Option<String>,

        /// Query GUID (requires --project).
        #[arg(long)]
        query_id: Option<String>,

        /// Number of IDs to skip (pagination).
        #[arg(long)]
        offset: Option<usize>,

        /// Maximum number of results to return (default: 200).
        #[arg(long)]
        limit: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let config = Config::load_with_overrides(cli.token.as_deref(), cli.base_url.as_deref())?;
    let modules = create_modules(&config)?;
    let work_items = modules.work_items;
    let links = modules.links;
    let comments = modules.comments;
    let projects = modules.projects;
    let queries = modules.queries;

    match cli.command {
        Command::Status {
            show_secrets,
            include_projects,
        } => {
            let result = projects
                .get_status(GetStatusParams {
                    show_secrets: Some(show_secrets),
                    include_projects: Some(include_projects),
                })
                .await?;
            format_status(&result, cli.json);
        }
        Command::ListProjects { offset, limit } => {
            let result = projects
                .list_projects(ListProjectsParams { offset, limit })
                .await?;
            format_list_projects(&result, cli.json);
        }
        Command::GetWorkItem {
            id,
            fields,
            expand,
            comments,
            description,
            format,
        } => {
            let result = work_items
                .get_work_item(GetWorkItemParams {
                    id: Some(parse_work_item_id(&id)),
                    fields,
                    expand,
                    include_comments: Some(comments),
                    include_description: Some(description),
                    relations_format: format.as_deref().map(|f| match f {
                        "urls" => RelationsFormat::Urls,
                        _ => RelationsFormat::Ids,
                    }),
                })
                .await?;
            format_work_item(&result, cli.json);
        }
        Command::CreateWorkItem {
            project,
            r#type,
            title,
            description,
            field_args,
            relations,
            comments,
        } => {
            let field_map: std::collections::BTreeMap<String, Value> =
                field_args.into_iter().collect();
            let params = build_create_work_item_params(
                project,
                r#type,
                title,
                description,
                field_map,
                relations,
                comments,
            )?;
            let result = work_items.create_work_item(params).await?;
            format_create_result(&result, cli.json);
        }
        Command::UpdateWorkItem {
            id,
            patch,
            expected_rev,
            field_args,
        } => {
            let patch_value: serde_json::Value = if !field_args.is_empty() || patch.is_some() {
                let mut ops: Vec<Value> = Vec::new();
                for (k, v) in &field_args {
                    let converted = if k == "System.Description" {
                        convert_description_value(v)
                    } else {
                        v.clone()
                    };
                    ops.push(
                        json!({"op":"replace","path": format!("/fields/{}",k),"value": converted}),
                    );
                }
                if let Some(p) = patch {
                    let parsed: Vec<Value> = serde_json::from_str(&p)
                        .map_err(|e| format!("invalid JSON patch: {}", e))?;
                    ops.extend(parsed);
                }
                serde_json::Value::Array(ops)
            } else {
                return Err("missing: provide --field KEY=VALUE or --patch JSON".into());
            };
            let result = work_items
                .update_work_item(UpdateWorkItemParams {
                    id: parse_work_item_id(&id),
                    patch: Some(patch_value),
                    tags: None,
                    expected_rev,
                })
                .await?;
            format_work_item(&result, cli.json);
        }
        Command::ManageLink {
            action,
            source_id,
            target,
            link_type,
            comment,
        } => {
            let attributes = comment.map(|c| json!({"comment": c}));
            let result = links
                .manage_work_item_link(ManageWorkItemLinkParams {
                    action,
                    source_id: parse_work_item_id(&source_id),
                    target: parse_work_item_id(&target),
                    link_type,
                    attributes,
                })
                .await?;
            format_link_result(&result, cli.json);
        }
        Command::Search {
            project,
            wiql,
            details,
            limit,
            description,
            format,
        } => {
            let result = work_items
                .search_work_items(SearchWorkItemsParams {
                    project,
                    wiql,
                    details: Some(details),
                    limit: Some(limit),
                    include_description: Some(description),
                    relations_format: format.as_deref().map(|f| match f {
                        "urls" => RelationsFormat::Urls,
                        _ => RelationsFormat::Ids,
                    }),
                })
                .await?;
            format_search(&result, cli.json);
        }
        Command::Comments { id } => {
            let result = comments
                .get_workitem_comments(GetWorkitemCommentsParams {
                    id: parse_work_item_id(&id),
                })
                .await?;
            format_comments(&result, cli.json);
        }
        Command::AddComment { id, text } => {
            let result = comments
                .add_comment(AddCommentParams {
                    id: parse_work_item_id(&id),
                    text,
                })
                .await?;
            format_add_comment_result(&result, cli.json);
        }
        Command::GetQueryResults {
            url,
            project,
            query_id,
            offset,
            limit,
        } => {
            let result = queries
                .get_query(GetQueryParams {
                    url,
                    project,
                    query_id,
                    offset,
                    limit,
                })
                .await?;
            format_query_results(&result, cli.json);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_relation_valid() {
        let r = parse_relation(
            "url=https://example.com/workItems/1,rel=System.LinkTypes.Related,comment=linked",
        )
        .unwrap();
        assert_eq!(r.url, "https://example.com/workItems/1");
        assert_eq!(r.rel, "System.LinkTypes.Related");
        assert_eq!(r.attributes.unwrap().comment.unwrap(), "linked");
    }

    #[test]
    fn parse_relation_minimal() {
        let r = parse_relation("url=https://example.com/workItems/1,rel=System.LinkTypes.Related")
            .unwrap();
        assert_eq!(r.url, "https://example.com/workItems/1");
        assert_eq!(r.rel, "System.LinkTypes.Related");
        assert!(r.attributes.is_none());
    }

    #[test]
    fn parse_relation_missing_url() {
        let err = parse_relation("rel=System.LinkTypes.Related").unwrap_err();
        assert!(err.contains("url"));
    }

    #[test]
    fn parse_relation_missing_rel() {
        let err = parse_relation("url=https://example.com/workItems/1").unwrap_err();
        assert!(err.contains("rel"));
    }

    #[test]
    fn parse_relation_unknown_key() {
        let err = parse_relation("url=https://example.com,rel=Foo,bogus=bar").unwrap_err();
        assert!(err.contains("bogus"));
    }

    #[test]
    fn parse_relation_no_equals() {
        let err = parse_relation("url,rel").unwrap_err();
        assert!(err.contains("key=value"));
    }
}
