use serde::{Deserialize, Serialize};
use unfour_core::models::{WorkspaceLayout, WorkspaceLayoutTab};
use unfour_core::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredWorkspaceLayout {
    sidebar_collapsed: bool,
    active_tab_id: String,
    tabs: Vec<WorkspaceLayoutTab>,
    selected_api_request_id: Option<String>,
    selected_database_connection_id: Option<String>,
    selected_ssh_connection_id: Option<String>,
    #[serde(default)]
    sidebar_width: i32,
    #[serde(default)]
    bottom_panel_height: i32,
    #[serde(default)]
    right_inspector_width: i32,
}

impl StoredWorkspaceLayout {
    pub(super) fn try_from_layout(workspace_id: &str, layout: WorkspaceLayout) -> AppResult<Self> {
        if layout.workspace_id != workspace_id {
            return Err(AppError::Validation(
                "layout workspace_id does not match command workspace_id".to_string(),
            ));
        }

        validate_layout_tabs(&layout.active_tab_id, &layout.tabs)?;

        Ok(Self {
            sidebar_collapsed: layout.sidebar_collapsed,
            active_tab_id: layout.active_tab_id,
            tabs: layout.tabs,
            selected_api_request_id: non_empty_optional(layout.selected_api_request_id),
            selected_database_connection_id: non_empty_optional(
                layout.selected_database_connection_id,
            ),
            selected_ssh_connection_id: non_empty_optional(layout.selected_ssh_connection_id),
            sidebar_width: layout.sidebar_width,
            bottom_panel_height: layout.bottom_panel_height,
            right_inspector_width: layout.right_inspector_width,
        })
    }
}

pub(super) fn parse_layout(workspace_id: &str, value: &str, updated_at: &str) -> WorkspaceLayout {
    let stored = serde_json::from_str::<StoredWorkspaceLayout>(value).unwrap_or_else(|_| {
        StoredWorkspaceLayout {
            sidebar_collapsed: false,
            active_tab_id: "api-main".to_string(),
            tabs: default_layout_tabs(),
            selected_api_request_id: None,
            selected_database_connection_id: None,
            selected_ssh_connection_id: None,
            sidebar_width: 0,
            bottom_panel_height: 0,
            right_inspector_width: 0,
        }
    });

    let mut tabs = stored.tabs;
    if validate_layout_tabs(&stored.active_tab_id, &tabs).is_err() {
        tabs = default_layout_tabs();
    }
    let active_tab_id = if tabs.iter().any(|tab| tab.id == stored.active_tab_id) {
        stored.active_tab_id
    } else {
        "api-main".to_string()
    };

    WorkspaceLayout {
        workspace_id: workspace_id.to_string(),
        sidebar_collapsed: stored.sidebar_collapsed,
        active_tab_id,
        tabs,
        selected_api_request_id: stored.selected_api_request_id,
        selected_database_connection_id: stored.selected_database_connection_id,
        selected_ssh_connection_id: stored.selected_ssh_connection_id,
        sidebar_width: if stored.sidebar_width > 0 {
            stored.sidebar_width
        } else {
            248
        },
        bottom_panel_height: if stored.bottom_panel_height > 0 {
            stored.bottom_panel_height
        } else {
            220
        },
        right_inspector_width: if stored.right_inspector_width > 0 {
            stored.right_inspector_width
        } else {
            300
        },
        updated_at: updated_at.to_string(),
    }
}

fn validate_layout_tabs(active_tab_id: &str, tabs: &[WorkspaceLayoutTab]) -> AppResult<()> {
    if tabs.is_empty() {
        return Err(AppError::Validation(
            "layout must include at least one tab".to_string(),
        ));
    }
    if active_tab_id.trim().is_empty() {
        return Err(AppError::Validation(
            "layout active_tab_id cannot be empty".to_string(),
        ));
    }

    for tab in tabs {
        if tab.id.trim().is_empty() || tab.title.trim().is_empty() {
            return Err(AppError::Validation(
                "layout tabs must have non-empty id and title".to_string(),
            ));
        }
        if !matches!(tab.kind.as_str(), "api" | "ssh" | "database") {
            return Err(AppError::Validation(format!(
                "unsupported layout tab kind: {}",
                tab.kind
            )));
        }
    }

    if !tabs.iter().any(|tab| tab.id == active_tab_id) {
        return Err(AppError::Validation(
            "layout active_tab_id must reference an open tab".to_string(),
        ));
    }

    Ok(())
}

fn default_layout_tabs() -> Vec<WorkspaceLayoutTab> {
    vec![
        WorkspaceLayoutTab {
            id: "api-main".to_string(),
            title: "API Client".to_string(),
            kind: "api".to_string(),
        },
        WorkspaceLayoutTab {
            id: "ssh-main".to_string(),
            title: "SSH Terminal".to_string(),
            kind: "ssh".to_string(),
        },
        WorkspaceLayoutTab {
            id: "database-main".to_string(),
            title: "Database".to_string(),
            kind: "database".to_string(),
        },
    ]
}

fn non_empty_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
