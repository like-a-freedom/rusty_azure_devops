use std::sync::Arc;

use crate::{
    AzureDevOpsClient, AzureError, Comment, OnPremFallback, Project, RetryStrategy, WorkItem,
    http_error::HttpError,
};
use azure_devops_rust_api::Credential;
use azure_devops_rust_api::wit;

/// Adapter that implements `AzureDevOpsClient` using `azure_devops_rust_api` and
/// separate retry + on-prem fallback strategies.
#[derive(Clone)]
pub struct AzureSdkClient {
    wit_client: wit::Client,
    core_client: azure_devops_rust_api::core::Client,
    org: String,
    project: Option<String>,
    pub(crate) base_contains_collection: bool,
    retry: Arc<dyn RetryStrategy>,
    fallback: Option<Arc<dyn OnPremFallback>>,
}

impl AzureSdkClient {
    /// Build an `AzureSdkClient`. Parses the endpoint URL, constructs SDK
    /// clients, and attaches the `DowngradeApiVersionPolicy` when the URL
    /// contains a collection path.
    pub fn builder(
        endpoint: impl Into<String>,
        pat: impl Into<String>,
        org: impl Into<String>,
    ) -> AzureSdkClientBuilder {
        AzureSdkClientBuilder {
            endpoint: endpoint.into(),
            pat: pat.into(),
            org: org.into(),
            project: None,
            api_version: None,
            retry: None,
            fallback: None,
            ca_cert: None,
        }
    }

    pub(crate) fn effective_org(&self) -> String {
        if self.base_contains_collection {
            "".to_string()
        } else {
            self.org.clone()
        }
    }

    fn effective_project(&self) -> String {
        self.project.clone().unwrap_or_default()
    }
}

/// Merge flat-query hits (`workItems`) with tree/oneHop hits
/// (`workItemRelations` source + target) into one deduplicated ID list.
/// Flat results come first; relation order is preserved after them.
/// `source` is `None` for top-level rows — only `target` carries the ID.
fn merge_query_result_ids(
    work_items: Vec<wit::models::WorkItemReference>,
    relations: Vec<wit::models::WorkItemLink>,
) -> Vec<i64> {
    use std::collections::HashSet;
    let mut seen: HashSet<i64> = HashSet::new();
    let mut out: Vec<i64> = Vec::new();
    for it in work_items {
        if let Some(id) = it.id {
            let id = id as i64;
            if seen.insert(id) {
                out.push(id);
            }
        }
    }
    for link in relations {
        for endpoint in [link.source, link.target].into_iter().flatten() {
            if let Some(id) = endpoint.id {
                let id = id as i64;
                if seen.insert(id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// Builder for `AzureSdkClient`. All fields are optional except the
/// constructor args (`endpoint`, `pat`, `org`) which are required.
pub struct AzureSdkClientBuilder {
    endpoint: String,
    pat: String,
    org: String,
    project: Option<String>,
    api_version: Option<String>,
    retry: Option<Arc<dyn RetryStrategy>>,
    fallback: Option<Arc<dyn OnPremFallback>>,
    ca_cert: Option<String>,
}

impl AzureSdkClientBuilder {
    /// Validate an Azure DevOps endpoint without constructing a client.
    /// HTTPS is required except for loopback HTTP endpoints used by local services.
    pub fn validate_endpoint(endpoint: &str) -> Result<(), AzureError> {
        crate::resilience::parse_secure_endpoint(endpoint).map(|_| ())
    }

    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = Some(api_version.into());
        self
    }

    /// Add a custom root CA certificate from a PEM or DER file.
    pub fn ca_cert(mut self, path: impl Into<String>) -> Self {
        self.ca_cert = Some(path.into());
        self
    }

    pub fn retry(mut self, retry: Arc<dyn RetryStrategy>) -> Self {
        self.retry = Some(retry);
        self
    }

    pub fn fallback(mut self, fallback: Arc<dyn OnPremFallback>) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Build an `AzureSdkClient` after validating the endpoint transport.
    pub fn build(self) -> Result<AzureSdkClient, AzureError> {
        let endpoint_s = self.endpoint.trim_end_matches('/').to_string();
        let pat_s = self.pat.clone();
        let cred = Credential::from_pat(pat_s.clone());
        let endpoint_url = crate::resilience::parse_secure_endpoint(&endpoint_s)?;
        let path = endpoint_url.path().to_lowercase();
        let base_contains_collection =
            path.contains("/tfs/") || path.contains("/defaultcollection");
        let api_versions = match self.api_version {
            Some(v) => vec![v],
            None => vec!["7.1-preview".into(), "7.0".into(), "6.0".into()],
        };

        let downgrade_policy: Vec<Arc<dyn azure_core::http::policies::Policy>> =
            if base_contains_collection {
                let target = api_versions
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "6.0".to_string());
                vec![Arc::new(
                    crate::api_version_policy::DowngradeApiVersionPolicy::new(target),
                )]
            } else {
                vec![]
            };
        let custom_http_client = self
            .ca_cert
            .as_deref()
            .map(build_ca_http_client)
            .transpose()?;

        let mut wit_builder = wit::Client::builder(cred.clone())
            .endpoint(endpoint_url.clone())
            .scopes(&["vso.work"])
            .per_try_policies(downgrade_policy.clone());
        if let Some(http_client) = custom_http_client.as_ref() {
            wit_builder = wit_builder.transport(azure_core::http::Transport::new(Arc::new(
                http_client.clone(),
            )));
        }
        let wit_client = wit_builder.build();

        let mut core_builder = azure_devops_rust_api::core::Client::builder(cred)
            .endpoint(endpoint_url)
            .scopes(&["vso.core"])
            .per_try_policies(downgrade_policy);
        if let Some(http_client) = custom_http_client.as_ref() {
            core_builder = core_builder.transport(azure_core::http::Transport::new(Arc::new(
                http_client.clone(),
            )));
        }
        let core_client = core_builder.build();

        let retry: Arc<dyn RetryStrategy> = self
            .retry
            .unwrap_or_else(|| Arc::new(crate::ProductionRetry::from_env()));
        let fallback: Option<Arc<dyn OnPremFallback>> = match self.fallback {
            Some(fallback) => Some(fallback),
            None if base_contains_collection => {
                let mut fallback = crate::ProductionFallback::new(endpoint_s, pat_s, api_versions)?;
                if let Some(http_client) = custom_http_client {
                    fallback = fallback.with_http_client(http_client);
                }
                Some(Arc::new(fallback))
            }
            None => None,
        };

        Ok(AzureSdkClient {
            wit_client,
            core_client,
            org: self.org,
            project: self.project,
            base_contains_collection,
            retry,
            fallback,
        })
    }
}

fn build_ca_http_client(path: &str) -> Result<reqwest::Client, AzureError> {
    let certificate_bytes = std::fs::read(path)
        .map_err(|_| AzureError::Config("cannot read custom CA certificate".to_string()))?;
    let certificate = reqwest::Certificate::from_pem(&certificate_bytes)
        .or_else(|_| reqwest::Certificate::from_der(&certificate_bytes))
        .map_err(|_| AzureError::Config("invalid custom CA certificate".to_string()))?;
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .add_root_certificate(certificate)
        .build()
        .map_err(|_| {
            AzureError::Config("cannot build HTTP client with custom CA certificate".to_string())
        })
}

fn work_item_id_to_sdk_id(id: i64) -> Result<i32, AzureError> {
    i32::try_from(id).map_err(|_| {
        AzureError::Http(HttpError::decode(
            "work item ID is outside the supported 32-bit range",
        ))
    })
}

/// Classify an `azure_core::Error` into `AzureError::Http(HttpError)`. When the
/// error is a `Decode` (the SDK failed to parse the on-prem response body),
/// return `Some(true)` so callers know to retry with an empty project
/// parameter — the on-prem heuristic preserved from ADR-0001 → ADR-0008.
///
/// Lives outside `AzureSdkClient` so it is callable from any retry closure
/// without a `self` borrow; classification has no state.
fn classify_sdk_error(e: azure_core::Error) -> (AzureError, bool) {
    use azure_core::error::ErrorKind;
    let kind = e.kind().clone();
    let display = e.to_string();
    let http = match kind {
        ErrorKind::HttpResponse { status, .. } => match u16::from(status) {
            404 => HttpError::NotFound,
            409 => HttpError::Conflict,
            _ => HttpError::Transport(format!("HTTP {status}: {display}")),
        },
        ErrorKind::DataConversion => HttpError::Decode(display),
        ErrorKind::Connection | ErrorKind::Io => HttpError::Transport(display),
        ErrorKind::Credential => HttpError::Transport(format!("credential: {display}")),
        ErrorKind::Other => HttpError::Transport(display),
    };
    let retry_with_empty_project = matches!(http, HttpError::Decode(_));
    (AzureError::Http(http), retry_with_empty_project)
}

/// Thin wrapper for sites that want the classified `AzureError` and don't need
/// the "decode → retry" flag.
fn sdk_error_to_azure(e: azure_core::Error) -> AzureError {
    classify_sdk_error(e).0
}

#[async_trait::async_trait]
impl AzureDevOpsClient for AzureSdkClient {
    async fn list_projects(&self) -> Result<Vec<Project>, AzureError> {
        if self.base_contains_collection {
            let val = self
                .fallback
                .as_ref()
                .expect("on-prem fallback required")
                .get("_apis/projects")
                .await?;
            let projects: Vec<Project> = val
                .get("value")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|p| Project {
                            id: p
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            name: p
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            return Ok(projects);
        }
        let org = self.effective_org();
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let core = self.core_client.clone();
                let org = org.clone();
                async move {
                    let req = core.projects_client().list(org.clone());
                    match req.into_future().await {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let (azure_err, _) = classify_sdk_error(e);
                            Err(azure_err)
                        }
                    }
                }
            })
            .await?;
        Ok(resp
            .value
            .into_iter()
            .map(|p| Project {
                id: p.id.unwrap_or_default(),
                name: p.name,
            })
            .collect())
    }

    async fn create_work_item(
        &self,
        project: &str,
        wi_type: &str,
        patch: &serde_json::Value,
    ) -> Result<WorkItem, AzureError> {
        let ops: Vec<wit::models::JsonPatchOperation> = serde_json::from_value(patch.clone())
            .map_err(|e| AzureError::Http(HttpError::decode(format!("invalid patch: {}", e))))?;
        let org = self.effective_org();
        let project_s = project.to_string();
        let type_s = wi_type.to_string();
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let ops = ops.clone();
                let project = project_s.clone();
                let type_ = type_s.clone();
                async move {
                    let req = wit.work_items_client().create(org, ops, project, type_);
                    req.into_future().await.map_err(sdk_error_to_azure)
                }
            })
            .await?;
        Ok(WorkItem {
            id: resp.id as i64,
            rev: resp.rev.map(|r| r as u64),
            fields: resp.fields,
            comments: None,
            relations: None,
        })
    }

    async fn get_work_item(
        &self,
        id: i64,
        fields: Option<&[&str]>,
        expand: Option<&str>,
    ) -> Result<WorkItem, AzureError> {
        let org = self.effective_org();
        let project = self.effective_project();
        let id32 = work_item_id_to_sdk_id(id)?;
        let fields_csv = fields.map(|f| f.join(","));
        let expand_val = expand.map(|s| s.to_string());

        let resp = if self.base_contains_collection {
            let mut path = format!("_apis/wit/workitems/{}", id32);
            let mut query_parts: Vec<String> = Vec::new();
            // on-prem TFS rejects $expand + fields together; drop fields when expand is set
            if expand_val.is_none()
                && let Some(ref fc) = fields_csv
            {
                query_parts.push(format!("fields={}", fc));
            }
            if let Some(ref ev) = expand_val {
                query_parts.push(format!("$expand={}", ev));
            }
            if !query_parts.is_empty() {
                path.push('?');
                path.push_str(&query_parts.join("&"));
            }
            let val = self
                .fallback
                .as_ref()
                .expect("on-prem fallback required")
                .get(&path)
                .await?;
            let w: wit::models::WorkItem =
                serde_json::from_value(val).map_err(|e| AzureError::Http(HttpError::decode(e)))?;
            w
        } else {
            self.retry
                .retry_policy()
                .retry_async(|| {
                    let wit = self.wit_client.clone();
                    let org = org.clone();
                    let project = project.clone();
                    let fc = fields_csv.clone();
                    async move {
                        let mut req = wit.work_items_client().get_work_item(
                            org.clone(),
                            id32,
                            project.clone(),
                        );
                        if let Some(ref f) = fc {
                            req = req.fields(f.as_str());
                        }
                        match req.into_future().await {
                            Ok(r) => Ok(r),
                            Err(e) => {
                                let (azure_err, retry_with_empty_project) = classify_sdk_error(e);
                                if retry_with_empty_project {
                                    let mut req2 = wit.work_items_client().get_work_item(
                                        org.clone(),
                                        id32,
                                        "".to_string(),
                                    );
                                    if let Some(ref f) = fc {
                                        req2 = req2.fields(f.as_str());
                                    }
                                    req2.into_future().await.map_err(|e2| {
                                        AzureError::Http(HttpError::transport(format!(
                                            "fallback failed: {}",
                                            e2
                                        )))
                                    })
                                } else {
                                    Err(azure_err)
                                }
                            }
                        }
                    }
                })
                .await?
        };

        let expand_str = expand.unwrap_or("");
        let want_relations =
            expand_str.eq_ignore_ascii_case("relations") || expand_str.eq_ignore_ascii_case("All");
        let relations = if want_relations && !resp.relations.is_empty() {
            Some(
                resp.relations
                    .into_iter()
                    .map(|r| crate::Relation {
                        rel: Some(r.link.rel),
                        url: r.link.url,
                        attributes: Some(r.link.attributes),
                    })
                    .collect(),
            )
        } else {
            None
        };

        let comments = if expand_str.eq_ignore_ascii_case("Comments")
            || expand_str.eq_ignore_ascii_case("All")
        {
            let org = self.effective_org();
            let project = self.effective_project();
            let creq = self
                .retry
                .retry_policy()
                .retry_async(|| {
                    let wit = self.wit_client.clone();
                    let org = org.clone();
                    let project = project.clone();
                    async move {
                        let req = wit.comments_client().get_comments(org, project, id32);
                        req.into_future().await.map_err(sdk_error_to_azure)
                    }
                })
                .await?;
            Some(
                creq.comments
                    .into_iter()
                    .map(|c| Comment {
                        id: c.id.map(|i| i as i64),
                        author: c
                            .created_by
                            .and_then(|r| r.graph_subject_base.display_name.clone()),
                        text: c.text.unwrap_or_default(),
                        created_at: c.created_date.map(|d| d.to_string()),
                    })
                    .collect(),
            )
        } else {
            None
        };

        Ok(WorkItem {
            id: resp.id as i64,
            rev: resp.rev.map(|r| r as u64),
            fields: resp.fields,
            comments,
            relations,
        })
    }

    async fn update_work_item(
        &self,
        id: i64,
        patch: &serde_json::Value,
        expected_rev: Option<u64>,
    ) -> Result<WorkItem, AzureError> {
        let mut patch = patch.clone();
        if let Some(expected_rev) = expected_rev
            && let Some(operations) = patch.as_array_mut()
        {
            operations.insert(
                0,
                serde_json::json!({
                    "op": "test",
                    "path": "/rev",
                    "value": expected_rev
                }),
            );
        }
        let ops: Vec<wit::models::JsonPatchOperation> = serde_json::from_value(patch)
            .map_err(|e| AzureError::Http(HttpError::decode(format!("invalid patch: {}", e))))?;
        let org = self.effective_org();
        let project = self.effective_project();
        let id32 = work_item_id_to_sdk_id(id)?;
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project.clone();
                let ops = ops.clone();
                async move {
                    let req = wit.work_items_client().update(
                        org.clone(),
                        ops.clone(),
                        id32,
                        project.clone(),
                    );
                    match req.into_future().await {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            let (azure_err, retry_with_empty_project) = classify_sdk_error(e);
                            if retry_with_empty_project {
                                let req2 = wit.work_items_client().update(
                                    org.clone(),
                                    ops.clone(),
                                    id32,
                                    "".to_string(),
                                );
                                req2.into_future().await.map_err(|e2| {
                                    AzureError::Http(HttpError::transport(format!(
                                        "fallback failed: {}",
                                        e2
                                    )))
                                })
                            } else {
                                Err(azure_err)
                            }
                        }
                    }
                }
            })
            .await?;
        Ok(WorkItem {
            id: resp.id as i64,
            rev: resp.rev.map(|r| r as u64),
            fields: resp.fields,
            comments: None,
            relations: None,
        })
    }

    async fn query_wiql(
        &self,
        project: &str,
        wiql: &str,
        _limit: usize,
    ) -> Result<Vec<i64>, AzureError> {
        let org = self.effective_org();
        let project_s = project.to_string();
        let body = wit::models::Wiql {
            query: Some(wiql.to_string()),
        };
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project_s.clone();
                let body = body.clone();
                async move {
                    let req = wit
                        .wiql_client()
                        .query_by_wiql(org, body, project, "".to_string());
                    match req.into_future().await {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let (azure_err, _) = classify_sdk_error(e);
                            Err(azure_err)
                        }
                    }
                }
            })
            .await?;
        Ok(resp
            .work_items
            .into_iter()
            .take(_limit)
            .filter_map(|it| it.id.map(|id| id as i64))
            .collect())
    }

    async fn query_by_id(&self, project: &str, query_id: &str) -> Result<Vec<i64>, AzureError> {
        let org = self.effective_org();
        let project_s = project.to_string();
        let id_s = query_id.to_string();
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project_s.clone();
                let id = id_s.clone();
                async move {
                    let req = wit
                        .wiql_client()
                        .query_by_id(org, id, project, "".to_string());
                    match req.into_future().await {
                        Ok(r) => Ok(r),
                        Err(e) => {
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            let (azure_err, _) = classify_sdk_error(e);
                            Err(azure_err)
                        }
                    }
                }
            })
            .await?;
        Ok(merge_query_result_ids(
            resp.work_items,
            resp.work_item_relations,
        ))
    }

    async fn batch_get_work_items(
        &self,
        ids: &[i64],
        fields: Option<&[&str]>,
    ) -> Result<Vec<WorkItem>, AzureError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let org = self.effective_org();
        let project = self.effective_project();
        let ids_i32: Vec<i32> = ids
            .iter()
            .map(|&id| work_item_id_to_sdk_id(id))
            .collect::<std::result::Result<_, _>>()?;
        let fields_owned: Option<Vec<String>> =
            fields.map(|f| f.iter().map(|s| s.to_string()).collect());
        let resp = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project.clone();
                let ids = ids_i32.clone();
                let fields = fields_owned.clone();
                async move {
                    let body = wit::models::WorkItemBatchGetRequest {
                        ids,
                        fields: fields.unwrap_or_default(),
                        ..Default::default()
                    };
                    let req = wit
                        .work_items_client()
                        .get_work_items_batch(org, body, project);
                    req.into_future().await.map_err(sdk_error_to_azure)
                }
            })
            .await?;
        Ok(resp
            .value
            .into_iter()
            .map(|w| WorkItem {
                id: w.id as i64,
                rev: w.rev.map(|r| r as u64),
                fields: w.fields,
                comments: None,
                relations: None,
            })
            .collect())
    }

    async fn get_comments(&self, work_item_id: i64) -> Result<Vec<Comment>, AzureError> {
        work_item_id_to_sdk_id(work_item_id)?;
        if self.base_contains_collection {
            self.get_comments_on_prem(work_item_id).await
        } else {
            self.get_comments_sdk(work_item_id).await
        }
    }

    async fn add_comment(
        &self,
        work_item_id: i64,
        comment: &Comment,
    ) -> Result<Comment, AzureError> {
        // On-prem must bypass the SDK: it sends `7.1-preview`, the downgrade
        // policy rewrites it to `6.0`, and the comments API accepts only
        // `6.0-preview`. That failure used to be invisible, because every
        // error fell through to the `System.History` patch below.
        if self.base_contains_collection
            && let Ok(created) = self.add_comment_on_prem(work_item_id, comment).await
        {
            return Ok(created);
        }

        let mut cc = wit::models::CommentCreate::new();
        cc.text = Some(comment.text.clone());
        let org = self.effective_org();
        let project = self.effective_project();
        let id32 = work_item_id_to_sdk_id(work_item_id)?;
        let resp_res: Result<wit::models::Comment, AzureError> = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project.clone();
                let cc = cc.clone();
                async move {
                    let req = wit.comments_client().add_comment(org, cc, project, id32);
                    req.into_future().await.map_err(sdk_error_to_azure)
                }
            })
            .await;

        match resp_res {
            Ok(resp) => Ok(Comment {
                id: resp.id.map(|i| i as i64),
                author: resp
                    .created_by
                    .and_then(|r| r.graph_subject_base.display_name.clone()),
                text: resp.text.unwrap_or_default(),
                created_at: resp.created_date.map(|d| d.to_string()),
            }),
            Err(_) => {
                let patch = serde_json::json!([ { "op": "add", "path": "/fields/System.History", "value": comment.text } ]);
                let _ = self
                    .update_work_item(work_item_id, &patch, None)
                    .await
                    .map_err(|e| {
                        AzureError::Http(HttpError::transport(format!(
                            "fallback update failed: {}",
                            e
                        )))
                    })?;
                Ok(Comment {
                    id: None,
                    author: None,
                    text: comment.text.clone(),
                    created_at: None,
                })
            }
        }
    }
}

impl AzureSdkClient {
    /// The project the work item comments API needs.
    ///
    /// `GET /{collection}/{project}/_apis/wit/workItems/{id}/comments` requires
    /// the project segment; without it the on-prem server answers 404, even
    /// when the work item has no comments. The configured default project wins.
    /// When there is none, ask the work item, which reports its project in
    /// `System.TeamProject`.
    async fn comments_project(&self, work_item_id: i64) -> Result<String, AzureError> {
        const TEAM_PROJECT: &str = "System.TeamProject";

        let configured = self.effective_project();
        if !configured.trim().is_empty() {
            return Ok(configured);
        }

        let wi = self
            .get_work_item(work_item_id, Some(&[TEAM_PROJECT]), None)
            .await?;
        wi.fields
            .get(TEAM_PROJECT)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                AzureError::Config(format!(
                    "the work item comments API needs a project, and work item {work_item_id} reports no {TEAM_PROJECT}. Set ADO_DEFAULT_PROJECT to a project name."
                ))
            })
    }

    /// Post a comment to the discussion thread on-prem.
    ///
    /// Mirrors [`Self::get_comments_on_prem`]: the comments API needs the
    /// project in the path and the `6.0-preview` api-version.
    async fn add_comment_on_prem(
        &self,
        work_item_id: i64,
        comment: &Comment,
    ) -> Result<Comment, AzureError> {
        let project = self.comments_project(work_item_id).await?;
        let path = format!("{}/_apis/wit/workItems/{}/comments", project, work_item_id);
        let body = serde_json::json!({ "text": comment.text });
        let val = self
            .fallback
            .as_ref()
            .expect("on-prem fallback required")
            .post_json(&path, "6.0-preview", &body)
            .await?;
        Ok(Comment {
            id: val.get("id").and_then(|v| v.as_i64()),
            author: val
                .get("createdBy")
                .and_then(|r| r.get("displayName"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            text: val
                .get("text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(comment.text.as_str())
                .to_string(),
            created_at: val
                .get("createdDate")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
    }

    /// On-prem: comments API requires `6.0-preview` but the SDK's
    /// `DowngradeApiVersionPolicy` rewrites `7.1-preview` → `6.0`.  Bypass
    /// the SDK and hit the REST endpoint directly via the fallback.
    async fn get_comments_on_prem(&self, work_item_id: i64) -> Result<Vec<Comment>, AzureError> {
        let project = self.comments_project(work_item_id).await?;
        let path = format!("{}/_apis/wit/workItems/{}/comments", project, work_item_id,);
        let val = self
            .fallback
            .as_ref()
            .expect("on-prem fallback required")
            .get_with_api_version(&path, "6.0-preview")
            .await?;
        let comments_arr = val
            .get("comments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let comments = comments_arr
            .into_iter()
            .map(|c| Comment {
                id: c.get("id").and_then(|v| v.as_i64()),
                author: c
                    .get("createdBy")
                    .and_then(|r| r.get("displayName"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                text: c
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: c
                    .get("createdDate")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            })
            .collect();
        Ok(comments)
    }

    async fn get_comments_sdk(&self, work_item_id: i64) -> Result<Vec<Comment>, AzureError> {
        let org = self.effective_org();
        let project = self.comments_project(work_item_id).await?;
        let id32 = work_item_id_to_sdk_id(work_item_id)?;
        let creq = self
            .retry
            .retry_policy()
            .retry_async(|| {
                let wit = self.wit_client.clone();
                let org = org.clone();
                let project = project.clone();
                async move {
                    let req = wit.comments_client().get_comments(org, project, id32);
                    req.into_future().await.map_err(sdk_error_to_azure)
                }
            })
            .await?;
        Ok(creq
            .comments
            .into_iter()
            .map(|c| Comment {
                id: c.id.map(|i| i as i64),
                author: c
                    .created_by
                    .and_then(|r| r.graph_subject_base.display_name.clone()),
                text: c.text.unwrap_or_default(),
                created_at: c.created_date.map(|d| d.to_string()),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_project_returns_default_empty() {
        let c = AzureSdkClient::builder("https://tfs.example.com", "fake", "")
            .build()
            .expect("valid HTTPS endpoint");
        assert_eq!(c.effective_project(), "");
    }

    #[test]
    fn effective_project_returns_stored_project() {
        let c = AzureSdkClient::builder("https://tfs.example.com", "fake", "")
            .project("MyProject")
            .build()
            .expect("valid HTTPS endpoint");
        assert_eq!(c.effective_project(), "MyProject");
    }

    #[test]
    fn classify_sdk_error_decodes_data_conversion_errors() {
        // DataConversion is what the Azure SDK emits when it fails to
        // deserialise an on-prem response body — the only trigger that should
        // cause the empty-project retry path.
        let parse_err = serde_json::from_str::<serde_json::Value>("not json")
            .unwrap_err()
            .into();
        let (err, retry) = classify_sdk_error(parse_err);
        assert!(
            matches!(err, AzureError::Http(crate::HttpError::Decode(_))),
            "expected Decode variant, got {err:?}"
        );
        assert!(retry, "Decode must signal retry-with-empty-project");
    }

    #[test]
    fn classify_sdk_error_classifies_404_and_409() {
        let not_found = azure_core::Error::with_message(
            azure_core::error::ErrorKind::HttpResponse {
                status: azure_core::http::StatusCode::NotFound,
                error_code: None,
                raw_response: None,
            },
            "missing",
        );
        let (err_404, retry_404) = classify_sdk_error(not_found);
        assert!(matches!(
            err_404,
            AzureError::Http(crate::HttpError::NotFound)
        ));
        assert!(
            !retry_404,
            "404 must not trigger the empty-project retry path"
        );

        let conflict = azure_core::Error::with_message(
            azure_core::error::ErrorKind::HttpResponse {
                status: azure_core::http::StatusCode::Conflict,
                error_code: None,
                raw_response: None,
            },
            "rev mismatch",
        );
        let (err_409, _) = classify_sdk_error(conflict);
        assert!(matches!(
            err_409,
            AzureError::Http(crate::HttpError::Conflict)
        ));
    }

    #[test]
    fn validate_endpoint_rejects_embedded_credentials() {
        let result = AzureSdkClientBuilder::validate_endpoint(
            "https://user:synthetic-password@tfs.example.invalid/tfs/DefaultCollection",
        );

        assert!(result.is_err());
    }

    #[test]
    fn production_fallback_rejects_remote_http_endpoint() {
        let result = crate::ProductionFallback::new(
            "http://tfs.example.invalid/tfs/DefaultCollection",
            "synthetic-fallback-pat",
            vec!["6.0".into()],
        );

        assert!(result.is_err());
    }

    #[test]
    fn new_detects_base_collection_in_url() {
        let c =
            AzureSdkClient::builder("https://tfs.example.com/tfs/DefaultCollection", "fake", "")
                .build()
                .expect("valid HTTPS endpoint");
        assert!(c.base_contains_collection);
    }

    #[test]
    fn new_detects_no_collection_in_url() {
        let c = AzureSdkClient::builder("https://dev.azure.com/org", "fake", "org")
            .build()
            .expect("valid HTTPS endpoint");
        assert!(!c.base_contains_collection);
    }

    #[test]
    fn new_trims_trailing_slash() {
        let c =
            AzureSdkClient::builder("https://tfs.example.com/tfs/DefaultCollection/", "fake", "")
                .build()
                .expect("valid HTTPS endpoint");
        assert!(c.base_contains_collection);
    }

    #[test]
    fn new_with_custom_api_version() {
        let c = AzureSdkClient::builder("https://tfs.example.com", "fake", "")
            .api_version("7.0")
            .build()
            .expect("valid HTTPS endpoint");
        assert_eq!(c.retry.retry_policy().max_retries, 3);
    }

    #[test]
    fn effective_org_is_empty_when_base_contains_collection() {
        let c = AzureSdkClient::builder(
            "https://tfs.example.com/tfs/DefaultCollection",
            "fake",
            "org",
        )
        .build()
        .expect("valid HTTPS endpoint");
        assert!(c.base_contains_collection);
        assert_eq!(c.effective_org(), "");
    }

    #[test]
    fn effective_org_is_org_for_cloud_url() {
        let c = AzureSdkClient::builder("https://dev.azure.com/yourorg", "fake", "org")
            .build()
            .expect("valid HTTPS endpoint");
        assert!(!c.base_contains_collection);
        assert_eq!(c.effective_org(), "org");
    }
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod regression_tests {
    use super::*;
    use crate::AzureDevOpsClient;
    use wiremock::matchers::{body_json, method, path, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn update_work_item_sends_expected_revision_test() {
        let mock = MockServer::start().await;
        let expected_patch = serde_json::json!([
            {"op": "test", "path": "/rev", "value": 7},
            {"op": "replace", "path": "/fields/System.Title", "value": "updated"}
        ]);
        Mock::given(method("PATCH"))
            .and(body_json(expected_patch))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 42,
                "rev": 8,
                "fields": {"System.Title": "updated"},
                "url": format!("{}/_apis/wit/workItems/42", mock.uri())
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(mock.uri(), "fake-pat", "org")
            .build()
            .expect("loopback HTTP mock endpoint");
        let patch = serde_json::json!([
            {"op": "replace", "path": "/fields/System.Title", "value": "updated"}
        ]);
        let result = client.update_work_item(42, &patch, Some(7)).await;

        assert!(
            result.is_ok(),
            "expected update to match request contract: {result:?}"
        );
    }

    #[tokio::test]
    async fn get_work_item_rejects_id_outside_sdk_range() {
        let mock = MockServer::start().await;
        let client = AzureSdkClient::builder(mock.uri(), "fake-pat", "org")
            .build()
            .expect("loopback HTTP mock endpoint");

        for id in [i64::from(i32::MAX) + 1, i64::from(i32::MIN) - 1] {
            let error = client
                .get_work_item(id, None, None)
                .await
                .expect_err("out-of-range work item IDs must be rejected before the request");
            assert!(
                matches!(error, AzureError::Http(HttpError::Decode(_))),
                "expected an ID validation error, got {error:?}"
            );
        }
    }

    #[tokio::test]
    async fn get_work_item_on_prem_with_fields_constructs_valid_url() {
        let mock = MockServer::start().await;
        let endpoint = format!("{}/tfs/DefaultCollection", mock.uri());

        Mock::given(method("GET"))
            .and(path("/tfs/DefaultCollection/_apis/wit/workitems/9579673"))
            .and(query_param("fields", "System.Id,System.Tags"))
            .and(query_param("api-version", "6.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 9579673,
                "fields": {"System.Title": "Test"},
                "url": format!("{}/_apis/wit/workItems/9579673", mock.uri())
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(endpoint, "fake-pat", "")
            .api_version("6.0")
            .build()
            .expect("loopback HTTP mock endpoint");
        let result = client
            .get_work_item(9579673, Some(&["System.Id", "System.Tags"]), None)
            .await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    }

    #[tokio::test]
    async fn get_work_item_on_prem_without_fields_constructs_valid_url() {
        let mock = MockServer::start().await;
        let endpoint = format!("{}/tfs/DefaultCollection", mock.uri());

        Mock::given(method("GET"))
            .and(path("/tfs/DefaultCollection/_apis/wit/workitems/42"))
            .and(query_param("api-version", "6.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 42,
                "fields": {"System.Title": "No Fields"},
                "url": format!("{}/_apis/wit/workItems/42", mock.uri())
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(endpoint, "fake-pat", "")
            .api_version("6.0")
            .build()
            .expect("loopback HTTP mock endpoint");
        let result = client.get_work_item(42, None, None).await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    }

    #[tokio::test]
    async fn list_projects_on_prem() {
        let mock = MockServer::start().await;
        let endpoint = format!("{}/tfs/DefaultCollection", mock.uri());

        Mock::given(method("GET"))
            .and(path("/tfs/DefaultCollection/_apis/projects"))
            .and(query_param("api-version", "6.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [
                    {"id": "p1", "name": "Project One"},
                    {"id": "p2", "name": "Project Two"}
                ]
            })))
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(endpoint, "fake-pat", "")
            .api_version("6.0")
            .build()
            .expect("loopback HTTP mock endpoint");
        let result = client.list_projects().await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Project One");
        assert_eq!(projects[1].name, "Project Two");
    }

    #[tokio::test]
    async fn list_projects_on_prem_with_api_versions() {
        let mock = MockServer::start().await;
        let endpoint = format!("{}/tfs/DefaultCollection", mock.uri());

        // Without custom api-version, ProductionResilience tries 7.1-preview, 7.0, 6.0
        // Each with two auth variants.
        // Mount broad mock matching any GET on the projects endpoint (any api-version).
        Mock::given(method("GET"))
            .and(path("/tfs/DefaultCollection/_apis/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "value": [{"id": "p1", "name": "P1"}]
            })))
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(endpoint, "fake-pat", "")
            .build()
            .expect("loopback HTTP mock endpoint");
        let result = client.list_projects().await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "P1");
    }

    #[test]
    fn merge_query_ids_flat_only() {
        let items = vec![
            wit::models::WorkItemReference {
                id: Some(1),
                url: None,
            },
            wit::models::WorkItemReference {
                id: Some(2),
                url: None,
            },
        ];
        assert_eq!(merge_query_result_ids(items, vec![]), vec![1, 2]);
    }

    #[test]
    fn merge_query_ids_onehop_null_source() {
        // Shape seen on-prem for XDR Q1 2027 (oneHop): empty workItems,
        // relations with null source and IDs in target.
        let rel = |id: i32| wit::models::WorkItemLink {
            rel: None,
            source: None,
            target: Some(wit::models::WorkItemReference {
                id: Some(id),
                url: None,
            }),
        };
        let relations = vec![rel(8243728), rel(10729324)];
        assert_eq!(
            merge_query_result_ids(vec![], relations),
            vec![8243728, 10729324]
        );
    }

    #[test]
    fn merge_query_ids_dedups_flat_source_target() {
        let items = vec![wit::models::WorkItemReference {
            id: Some(7),
            url: None,
        }];
        let relations = vec![wit::models::WorkItemLink {
            rel: Some("System.LinkTypes.Hierarchy-Forward".to_string()),
            source: Some(wit::models::WorkItemReference {
                id: Some(7),
                url: None,
            }),
            target: Some(wit::models::WorkItemReference {
                id: Some(9),
                url: None,
            }),
        }];
        assert_eq!(merge_query_result_ids(items, relations), vec![7, 9]);
    }

    #[tokio::test]
    async fn query_by_id_merges_onehop_relation_ids() {
        // On-prem oneHop shape: empty `workItems`, IDs in
        // `workItemRelations` with null source on top-level rows.
        // Guards the query_by_id -> merge_query_result_ids wire-in:
        // collecting work_items only would return [] here.
        let mock = MockServer::start().await;
        let endpoint = format!("{}/tfs/DefaultCollection", mock.uri());

        Mock::given(method("GET"))
            .and(path_regex(r"/_apis/wit/wiql/q1$"))
            .and(query_param("api-version", "6.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "queryType": "oneHop",
                "queryResultType": "workItemLink",
                "workItems": [],
                "workItemRelations": [
                    {"rel": null, "source": null, "target": {"id": 8243728}},
                    {"rel": "System.LinkTypes.Hierarchy-Forward",
                     "source": {"id": 8243728},
                     "target": {"id": 10729324}}
                ]
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = AzureSdkClient::builder(endpoint, "fake-pat", "")
            .api_version("6.0")
            .build()
            .expect("loopback HTTP mock endpoint");
        let result = client.query_by_id("Corp-SecurityCenter", "q1").await;
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert_eq!(result.unwrap(), vec![8243728, 10729324]);
    }
}
