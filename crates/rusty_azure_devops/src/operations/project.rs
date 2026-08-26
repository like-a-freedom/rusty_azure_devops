use std::sync::Arc;

use azure_devops_client::{AzureDevOpsClient, AzureError};

use crate::domain::*;

use super::ServiceConfig;

/// Project-level queries: list projects and server status.
/// Two-method interface — thin by nature.
#[derive(Clone)]
pub struct ProjectLookup {
    client: Arc<dyn AzureDevOpsClient>,
    config: ServiceConfig,
}

impl ProjectLookup {
    pub fn new(client: Arc<dyn AzureDevOpsClient>) -> Self {
        Self {
            client,
            config: ServiceConfig::default(),
        }
    }

    pub fn with_config(client: Arc<dyn AzureDevOpsClient>, config: ServiceConfig) -> Self {
        Self { client, config }
    }

    pub async fn list_projects(
        &self,
        p: ListProjectsParams,
    ) -> Result<ListProjectsResult, AzureError> {
        let all = self.client.list_projects().await?;
        let total = all.len();
        let offset = p.offset.unwrap_or(0);
        let limit = p.limit.unwrap_or(total);
        let mapped: Vec<ProjectDto> = all
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|proj| proj.into())
            .collect();
        Ok(ListProjectsResult {
            projects: mapped,
            total,
        })
    }

    pub async fn get_status(&self, p: GetStatusParams) -> Result<StatusResult, AzureError> {
        let projects = self.client.list_projects().await?;

        let include_projects = p.include_projects.unwrap_or(false);
        let project_count = projects.len();

        let ps: Vec<ProjectDto> = if include_projects {
            projects.into_iter().map(|p| p.into()).collect()
        } else {
            vec![]
        };

        let mut config_dto: Option<StatusConfigDto> = None;
        if p.show_secrets.unwrap_or(false) {
            config_dto = Some(StatusConfigDto {
                token_present: self.config.token_present,
                ado_base_url: self.config.base_url.clone(),
                ado_default_project: self.config.default_project.clone(),
            });
        }

        Ok(StatusResult {
            status: "ok".into(),
            project_count,
            projects: ps,
            default_project: self.config.default_project.clone(),
            config: config_dto,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn service_get_status() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = ProjectLookup::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_status(GetStatusParams {
                show_secrets: None,
                include_projects: None,
            })
            .await
            .unwrap();
        assert_eq!(res.status, "ok");
        assert_eq!(res.project_count, 1);
        assert!(res.projects.is_empty());
    }

    #[tokio::test]
    async fn service_get_status_with_projects() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = ProjectLookup::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .get_status(GetStatusParams {
                show_secrets: None,
                include_projects: Some(true),
            })
            .await
            .unwrap();
        assert_eq!(res.projects.len(), 1);
    }

    #[tokio::test]
    async fn service_list_projects() {
        use azure_devops_client::InMemoryAzureClient;
        let svc = ProjectLookup::new(Arc::new(InMemoryAzureClient::default()));
        let res = svc
            .list_projects(ListProjectsParams {
                offset: None,
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(res.total, 1);
        assert_eq!(res.projects[0].name, "Demo");
    }
}
