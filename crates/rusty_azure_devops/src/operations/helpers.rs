use azure_devops_client::{AzureError, HttpError};

use crate::domain::*;

pub(crate) fn rewrite_missing_fields_error(e: AzureError) -> AzureError {
    match &e {
        AzureError::Http(HttpError::Decode(s)) if s.contains("missing field `fields`") => {
            AzureError::Http(HttpError::decode(format!(
                "Azure error: missing field `fields` when updating work item. This often means the server expected required `fields` (for create) or the patch was malformed. To add a relation to an existing work item, use a JSON Patch like: {{\"id\":123,\"patch\":[{{\"op\":\"add\",\"path\":\"/relations/-\",\"value\":{{\"rel\":\"System.LinkTypes.Related\",\"url\":\"https://dev.azure.com/ORG/_apis/wit/workItems/<id>\"}}}}]}}. Original error: {}",
                s
            )))
        }
        _ => e,
    }
}

pub(crate) fn normalize_description_fields(wf: &mut WorkItemFields) {
    if let Some(ref mut extra) = wf.extra
        && let Some(desc_val) = extra.get_mut("System.Description")
        && let Some(html) = desc_val.as_str()
    {
        *desc_val = serde_json::Value::String(html_to_markdown(html));
    }
    if let Some(ref mut desc) = wf.description {
        *desc = html_to_markdown(desc);
    }
}
