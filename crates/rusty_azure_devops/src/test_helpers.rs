use std::sync::Arc;

use azure_devops_client::AzureDevOpsClient;

/// A client whose `update_work_item` returns "missing field `fields`" error.
/// Used to verify that `rewrite_missing_fields_error` in helpers.rs wraps it correctly.
pub(crate) struct MissingFieldsClient;

#[async_trait::async_trait]
impl AzureDevOpsClient for MissingFieldsClient {
    async fn list_projects(
        &self,
    ) -> Result<Vec<azure_devops_client::Project>, azure_devops_client::AzureError> {
        Ok(vec![])
    }
    async fn create_work_item(
        &self,
        _project: &str,
        _wi_type: &str,
        _patch: &serde_json::Value,
    ) -> Result<azure_devops_client::WorkItem, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn get_work_item(
        &self,
        _id: i64,
        _fields: Option<&[&str]>,
        _expand: Option<&str>,
    ) -> Result<azure_devops_client::WorkItem, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn update_work_item(
        &self,
        _id: i64,
        _patch: &serde_json::Value,
        _expected_rev: Option<u64>,
    ) -> Result<azure_devops_client::WorkItem, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::Http(
            azure_devops_client::HttpError::decode("missing field `fields`"),
        ))
    }
    async fn query_wiql(
        &self,
        _project: &str,
        _wiql: &str,
        _limit: usize,
    ) -> Result<Vec<i64>, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn query_by_id(
        &self,
        _project: &str,
        _query_id: &str,
    ) -> Result<Vec<i64>, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn batch_get_work_items(
        &self,
        _ids: &[i64],
        _fields: Option<&[&str]>,
    ) -> Result<Vec<azure_devops_client::WorkItem>, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn get_comments(
        &self,
        _work_item_id: i64,
    ) -> Result<Vec<azure_devops_client::Comment>, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
    async fn add_comment(
        &self,
        _work_item_id: i64,
        _comment: &azure_devops_client::Comment,
    ) -> Result<azure_devops_client::Comment, azure_devops_client::AzureError> {
        Err(azure_devops_client::AzureError::NotImplemented)
    }
}

pub(crate) fn missing_fields_client() -> Arc<MissingFieldsClient> {
    Arc::new(MissingFieldsClient)
}
