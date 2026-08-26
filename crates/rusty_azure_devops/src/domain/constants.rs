/// Minimal fields for listing/searching work items. Covers most agent needs.
pub const DEFAULT_LIST_FIELDS: &[&str] = &[
    "System.Id",
    "System.WorkItemType",
    "System.State",
    "System.Title",
    "System.AssignedTo",
];

/// Fields returned by get_work_item when no fields are explicitly requested.
pub const DEFAULT_GET_FIELDS: &[&str] = &[
    "System.Id",
    "System.Rev",
    "System.WorkItemType",
    "System.State",
    "System.Title",
    "System.AssignedTo",
    "System.CreatedDate",
    "System.ChangedDate",
    "System.Tags",
    "System.TeamProject",
];

/// Fields included when include_description=true is set on get_work_item.
pub const DESCRIPTION_FIELDS: &[&str] = &["System.Description"];
