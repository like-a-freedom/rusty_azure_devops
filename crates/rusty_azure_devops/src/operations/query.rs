use std::sync::Arc;

use azure_devops_client::{AzureDevOpsClient, AzureError, HttpError};

use crate::domain::*;

/// Query operations: run saved/shared queries by URL or (project, query_id).
/// Separate from WorkItemOperations so query-specific features can grow
/// independently (list shared queries, get query by name, etc.).
#[derive(Clone)]
pub struct QueryOperations {
    client: Arc<dyn AzureDevOpsClient>,
}

impl QueryOperations {
    pub fn new(client: Arc<dyn AzureDevOpsClient>) -> Self {
        Self { client }
    }

    pub async fn get_query(&self, p: GetQueryParams) -> Result<GetQueryResult, AzureError> {
        let (project, query_id) = resolve_query_params(
            p.url.as_deref(),
            p.project.as_deref(),
            p.query_id.as_deref(),
        )?;
        let offset = p.offset.unwrap_or(0);
        let limit = p.limit.unwrap_or(200);
        let all_ids = self.client.query_by_id(&project, &query_id).await?;
        let total = all_ids.len();
        let ids: Vec<i64> = all_ids.into_iter().skip(offset).take(limit).collect();
        Ok(GetQueryResult { ids, total })
    }
}

fn resolve_query_params(
    url: Option<&str>,
    project: Option<&str>,
    query_id: Option<&str>,
) -> Result<(String, String), AzureError> {
    match (url, project, query_id) {
        (Some(u), _, _) => parse_query_url(u),
        (None, Some(p), Some(q)) => Ok((p.to_string(), q.to_string())),
        (None, Some(_), None) => Err(AzureError::Http(HttpError::decode(
            "query_id is required when project is provided. Provide query_id or pass a full query URL instead.",
        ))),
        (None, None, Some(_)) => Err(AzureError::Http(HttpError::decode(
            "project is required when query_id is provided. Provide project or pass a full query URL instead.",
        ))),
        (None, None, None) => Err(AzureError::Http(HttpError::decode(
            "provide either a query URL (e.g. .../Project/_queries/query/GUID) or (project, query_id). Example URL: https://tfs.example.com/tfs/Collection/MyProject/_queries/query/1c3ed7b3-17bb-4fe9-9b64-256f118ebebf",
        ))),
    }
}

fn parse_query_url(url_str: &str) -> Result<(String, String), AzureError> {
    let parsed = url::Url::parse(url_str).map_err(|_| {
        AzureError::Http(HttpError::decode(format!("invalid query URL: {}", url_str)))
    })?;
    let path = parsed.path();

    let marker = "/_queries/query/";
    let idx = path.find(marker).ok_or_else(|| {
        AzureError::Http(HttpError::decode(format!(
            "URL does not contain '/_queries/query/' segment: {}. Expected format: https://host/Project/_queries/query/QUERY_GUID",
            url_str
        )))
    })?;

    let project = path[..idx].rsplit('/').next().ok_or_else(|| {
        AzureError::Http(HttpError::decode(format!(
            "cannot extract project from URL: {}",
            url_str
        )))
    })?;

    let query_id = path[idx + marker.len()..].trim_end_matches('/');
    if project.is_empty() || query_id.is_empty() {
        return Err(AzureError::Http(HttpError::decode(format!(
            "invalid query URL: {} (empty project or query id)",
            url_str
        ))));
    }

    Ok((project.to_string(), query_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_url_valid() {
        let result = parse_query_url(
            "https://tfs.example.com/tfs/Collection/MyProject/_queries/query/1c3ed7b3-17bb-4fe9-9b64-256f118ebebf",
        );
        let (project, query_id) = result.unwrap();
        assert_eq!(project, "MyProject");
        assert_eq!(query_id, "1c3ed7b3-17bb-4fe9-9b64-256f118ebebf");
    }

    #[test]
    fn test_parse_query_url_no_queries_segment() {
        let result =
            parse_query_url("https://tfs.example.com/tfs/Collection/MyProject/something/else");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_query_url_invalid() {
        let result = parse_query_url("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_query_params_url() {
        let result = resolve_query_params(
            Some(
                "https://tfs.example.com/tfs/Collection/MyProject/_queries/query/1c3ed7b3-17bb-4fe9-9b64-256f118ebebf",
            ),
            None,
            None,
        );
        let (project, query_id) = result.unwrap();
        assert_eq!(project, "MyProject");
        assert_eq!(query_id, "1c3ed7b3-17bb-4fe9-9b64-256f118ebebf");
    }

    #[test]
    fn test_resolve_query_params_project_and_id() {
        let result = resolve_query_params(None, Some("MyProject"), Some("query-id-123"));
        let (project, query_id) = result.unwrap();
        assert_eq!(project, "MyProject");
        assert_eq!(query_id, "query-id-123");
    }

    #[test]
    fn test_resolve_query_params_project_only() {
        let result = resolve_query_params(None, Some("MyProject"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_query_params_id_only() {
        let result = resolve_query_params(None, None, Some("query-id-123"));
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_query_params_nothing() {
        let result = resolve_query_params(None, None, None);
        assert!(result.is_err());
    }
}
