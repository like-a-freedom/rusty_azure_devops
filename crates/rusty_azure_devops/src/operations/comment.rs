use std::sync::Arc;

use azure_devops_client::{AzureDevOpsClient, AzureError, Comment};

use crate::domain::*;

/// Manages work item discussion comments — get and add.
/// Takes only `Arc<dyn AzureDevOpsClient>` — no config needed
/// (comment operations don't require base_url or default_project).
#[derive(Clone)]
pub struct CommentOperations {
    client: Arc<dyn AzureDevOpsClient>,
}

impl CommentOperations {
    pub fn new(client: Arc<dyn AzureDevOpsClient>) -> Self {
        Self { client }
    }

    pub async fn get_workitem_comments(
        &self,
        p: GetWorkitemCommentsParams,
    ) -> Result<GetWorkitemCommentsResult, AzureError> {
        let id = p.id.to_i64()?;
        let comments = self.client.get_comments(id).await?;
        let total = comments.len();
        let dtos: Vec<CommentDto> = comments.into_iter().map(|c| c.into()).collect();
        Ok(GetWorkitemCommentsResult {
            id,
            comments: dtos,
            total,
        })
    }

    pub async fn add_comment(&self, p: AddCommentParams) -> Result<AddCommentResult, AzureError> {
        let id = p.id.to_i64()?;
        let client_comment = Comment {
            id: None,
            author: None,
            text: p.text.clone(),
            created_at: None,
        };
        let result = self.client.add_comment(id, &client_comment).await?;
        Ok(AddCommentResult {
            id,
            comment_id: result.id,
            text: result.text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn comment_ops_get_comments() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = CommentOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_workitem_comments(GetWorkitemCommentsParams {
                id: WorkItemId::Int(123),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        assert_eq!(res.total, 1);
    }

    #[tokio::test]
    async fn comment_ops_add_comment() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = CommentOperations::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .add_comment(AddCommentParams {
                id: WorkItemId::Int(123),
                text: "test comment".into(),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        assert_eq!(res.text, "test comment");
    }

    #[tokio::test]
    async fn comment_ops_add_comment_returns_input_text() {
        use azure_devops_client::InMemoryAzureClient;
        let client = InMemoryAzureClient::default();
        {
            let mut comments = client.comments.lock().unwrap();
            comments
                .entry(123)
                .or_default()
                .push(azure_devops_client::Comment {
                    id: Some(99),
                    author: Some("Tester".into()),
                    text: "existing".into(),
                    created_at: None,
                });
        }
        let svc = CommentOperations::new(Arc::new(client));
        let res = svc
            .add_comment(AddCommentParams {
                id: WorkItemId::Int(123),
                text: "new comment".into(),
            })
            .await
            .unwrap();
        assert_eq!(res.id, 123);
        assert_eq!(res.text, "new comment");
    }
}
