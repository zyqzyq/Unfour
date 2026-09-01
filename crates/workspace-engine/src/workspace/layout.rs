use serde::{Deserialize, Serialize};
use unfour_core::models::{WorkspaceLayout, WorkspaceLayoutTab, WorkspaceSidebarWidths};
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
    sidebar_widths: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sidebar_width: Option<serde_json::Value>,
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

        let sidebar_widths = if layout.sidebar_width.is_some()
            && layout.sidebar_widths == WorkspaceSidebarWidths::default()
        {
            migrate_legacy_sidebar_width(layout.sidebar_width)
        } else {
            layout.sidebar_widths
        };

        Ok(Self {
            sidebar_collapsed: layout.sidebar_collapsed,
            active_tab_id: layout.active_tab_id,
            tabs: layout.tabs,
            selected_api_request_id: non_empty_optional(layout.selected_api_request_id),
            selected_database_connection_id: non_empty_optional(
                layout.selected_database_connection_id,
            ),
            selected_ssh_connection_id: non_empty_optional(layout.selected_ssh_connection_id),
            sidebar_widths: Some(serde_json::to_value(sidebar_widths)?),
            sidebar_width: None,
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
            sidebar_widths: None,
            sidebar_width: None,
            bottom_panel_height: 0,
            right_inspector_width: 0,
        }
    });

    let sidebar_widths = parse_sidebar_widths(
        stored.sidebar_widths.as_ref(),
        stored.sidebar_width.as_ref(),
    );

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
        sidebar_widths,
        sidebar_width: None,
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

fn parse_sidebar_widths(
    sidebar_widths: Option<&serde_json::Value>,
    legacy_width: Option<&serde_json::Value>,
) -> WorkspaceSidebarWidths {
    let defaults = WorkspaceSidebarWidths::default();
    if let Some(widths) = sidebar_widths.and_then(serde_json::Value::as_object) {
        return WorkspaceSidebarWidths {
            api: parse_width(widths.get("api")).unwrap_or(defaults.api),
            ssh: parse_width(widths.get("ssh")).unwrap_or(defaults.ssh),
            database: parse_width(widths.get("database")).unwrap_or(defaults.database),
        };
    }

    if let Some(legacy_width) = parse_width(legacy_width) {
        return WorkspaceSidebarWidths {
            api: legacy_width,
            ssh: legacy_width,
            database: legacy_width,
        };
    }

    defaults
}

fn migrate_legacy_sidebar_width(width: Option<i32>) -> WorkspaceSidebarWidths {
    let width = width.unwrap_or_else(|| WorkspaceSidebarWidths::default().api);
    WorkspaceSidebarWidths {
        api: width,
        ssh: width,
        database: width,
    }
}

fn parse_width(value: Option<&serde_json::Value>) -> Option<i32> {
    value
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_layout_migrates_legacy_width_without_resetting_layout_state() {
        let value = serde_json::json!({
            "sidebarCollapsed": true,
            "activeTabId": "ssh-main",
            "tabs": [
                { "id": "api-main", "title": "API Client", "kind": "api" },
                { "id": "ssh-main", "title": "SSH Terminal", "kind": "ssh" }
            ],
            "selectedApiRequestId": "request-1",
            "selectedDatabaseConnectionId": null,
            "selectedSshConnectionId": "connection-1",
            "sidebarWidth": 500,
            "bottomPanelHeight": 240,
            "rightInspectorWidth": 320
        });

        let layout = parse_layout("workspace-1", &value.to_string(), "updated");

        assert!(layout.sidebar_collapsed);
        assert_eq!(layout.active_tab_id, "ssh-main");
        assert_eq!(layout.selected_api_request_id.as_deref(), Some("request-1"));
        assert_eq!(layout.sidebar_widths.api, 500);
        assert_eq!(layout.sidebar_widths.ssh, 500);
        assert_eq!(layout.sidebar_widths.database, 500);
        assert_eq!(layout.bottom_panel_height, 240);
        assert_eq!(layout.right_inspector_width, 320);
    }

    #[test]
    fn parse_layout_uses_module_defaults_for_invalid_new_widths() {
        let value = serde_json::json!({
            "sidebarCollapsed": false,
            "activeTabId": "api-main",
            "tabs": [
                { "id": "api-main", "title": "API Client", "kind": "api" }
            ],
            "selectedApiRequestId": null,
            "selectedDatabaseConnectionId": null,
            "selectedSshConnectionId": null,
            "sidebarWidths": {
                "api": null,
                "ssh": "invalid",
                "database": 700
            }
        });

        let layout = parse_layout("workspace-1", &value.to_string(), "updated");

        assert_eq!(layout.sidebar_widths.api, 320);
        assert_eq!(layout.sidebar_widths.ssh, 248);
        assert_eq!(layout.sidebar_widths.database, 700);
    }
}
