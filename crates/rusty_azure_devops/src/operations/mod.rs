mod comment;
mod helpers;
mod link;
mod project;
mod query;
mod work_item;

pub use comment::CommentOperations;
pub use link::LinkOperations;
pub use project::ProjectLookup;
pub use query::QueryOperations;
pub use work_item::WorkItemOperations;

#[derive(Debug, Clone, Default)]
pub struct ServiceConfig {
    pub base_url: Option<String>,
    pub default_project: Option<String>,
    pub token_present: bool,
}
