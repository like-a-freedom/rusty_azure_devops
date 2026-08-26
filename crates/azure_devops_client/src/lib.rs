//! Azure DevOps client trait and thin wrapper.

pub mod http_error;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use thiserror::Error;

pub use http_error::HttpError;
pub use http_error_ext::AzureErrorExt;

#[derive(Debug, Clone, Error)]
pub enum AzureError {
    #[error("{0}")]
    Http(HttpError),
    #[error("{0}")]
    Config(String),
    #[error("not implemented")]
    NotImplemented,
}

mod http_error_ext;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relation {
    pub rel: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkItem {
    pub id: i64,
    pub rev: Option<u64>,
    pub fields: serde_json::Value,
    pub comments: Option<Vec<Comment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<Vec<Relation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: Option<i64>,
    pub author: Option<String>,
    pub text: String,
    pub created_at: Option<String>,
}

#[async_trait]
pub trait AzureDevOpsClient: Send + Sync + 'static {
    async fn list_projects(&self) -> Result<Vec<Project>, AzureError>;
    async fn create_work_item(
        &self,
        project: &str,
        wi_type: &str,
        patch: &serde_json::Value,
    ) -> Result<WorkItem, AzureError>;
    async fn get_work_item(
        &self,
        id: i64,
        fields: Option<&[&str]>,
        expand: Option<&str>,
    ) -> Result<WorkItem, AzureError>;
    async fn update_work_item(
        &self,
        id: i64,
        patch: &serde_json::Value,
        expected_rev: Option<u64>,
    ) -> Result<WorkItem, AzureError>;
    async fn query_wiql(
        &self,
        project: &str,
        wiql: &str,
        limit: usize,
    ) -> Result<Vec<i64>, AzureError>;
    async fn query_by_id(&self, project: &str, query_id: &str) -> Result<Vec<i64>, AzureError>;
    async fn batch_get_work_items(
        &self,
        ids: &[i64],
        fields: Option<&[&str]>,
    ) -> Result<Vec<WorkItem>, AzureError>;
    async fn get_comments(&self, work_item_id: i64) -> Result<Vec<Comment>, AzureError>;
    async fn add_comment(
        &self,
        work_item_id: i64,
        comment: &Comment,
    ) -> Result<Comment, AzureError>;
}

mod api_version_policy;
mod resilience;

// SDK-backed client (always enabled)
mod client;

pub use client::*;

mod in_memory_client;
pub use in_memory_client::InMemoryAzureClient;

// Re-export resilience strategy + adapters + retry policy
pub use resilience::{
    NoOpFallback, NoOpRetry, OnPremFallback, ProductionFallback, ProductionRetry, RetryPolicy,
    RetryStrategy,
};

#[cfg(test)]
mod tests {
    use super::*;
    use azure_core::Error as SdkError;
    use azure_core::error::ErrorKind;
    use azure_core::http::StatusCode;

    #[tokio::test]
    async fn mock_list_projects() {
        let c = InMemoryAzureClient::default();
        let p = c.list_projects().await.unwrap();
        assert_eq!(p.len(), 1);
    }

    #[tokio::test]
    async fn mock_create_get_update() {
        let c = InMemoryAzureClient::default();
        let created = c
            .create_work_item("Demo", "Task", &serde_json::json!([]))
            .await
            .unwrap();
        assert_eq!(created.id, 123);
        let got = c.get_work_item(123, None, None).await.unwrap();
        assert_eq!(got.id, 123);
        let updated = c
            .update_work_item(123, &serde_json::json!([]), Some(1))
            .await
            .unwrap();
        assert_eq!(updated.rev.unwrap(), 2);
    }

    #[tokio::test]
    async fn mock_get_with_comments() {
        let c = InMemoryAzureClient::default();
        let got = c.get_work_item(123, None, Some("Comments")).await.unwrap();
        assert!(got.comments.is_some());
        let comments = got.comments.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "hello");
    }

    #[test]
    fn http_error_status_404_classifies_as_not_found() {
        let err = SdkError::with_message(
            ErrorKind::HttpResponse {
                status: StatusCode::NotFound,
                error_code: Some("NotFound".into()),
                raw_response: None,
            },
            "resource gone",
        );
        assert!(matches!(
            err.into_azure(),
            AzureError::Http(HttpError::NotFound)
        ));
    }

    #[test]
    fn http_error_status_409_classifies_as_conflict() {
        let err = SdkError::with_message(
            ErrorKind::HttpResponse {
                status: StatusCode::Conflict,
                error_code: Some("Conflict".into()),
                raw_response: None,
            },
            "rev mismatch",
        );
        assert!(matches!(
            err.into_azure(),
            AzureError::Http(HttpError::Conflict)
        ));
    }

    #[test]
    fn http_error_data_conversion_classifies_as_decode() {
        // Matches the on-prem SDK-parse-error shape: Azure SDK failed to
        // deserialise the response body.
        let err = SdkError::with_message(
            ErrorKind::DataConversion,
            "missing field `fields` at line 1 column 32",
        );
        assert!(matches!(
            err.into_azure(),
            AzureError::Http(HttpError::Decode(_))
        ));
    }

    #[test]
    fn http_error_connection_classifies_as_transport() {
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "no route");
        let err = SdkError::new(ErrorKind::Connection, io);
        assert!(matches!(
            err.into_azure(),
            AzureError::Http(HttpError::Transport(_))
        ));
    }

    #[test]
    fn azure_error_remains_clone() {
        // Regression guard: `clone_error` round-tripping through Display was
        // deleted once `AzureError` derived `Clone`; the enum must stay
        // clonable so `InMemoryAzureClient` can hand out the same error both
        // as the original call result and from the per-method error slot.
        let err = AzureError::Http(HttpError::Conflict);
        let _cloned = err.clone();
    }

    #[test]
    fn http_error_fallback_variant_preserves_attempt_messages() {
        let err = HttpError::Fallback {
            errors: vec!["6.0 -> 401".into(), "7.0 -> 404".into()],
        };
        let s = err.to_string();
        assert!(s.contains("6.0 -> 401"));
        assert!(s.contains("7.0 -> 404"));
    }
}
