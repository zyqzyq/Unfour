use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub last_opened_at: Option<String>,
    pub environment_type: String,
    pub mcp_policy: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
    pub sync_status: String,
    pub remote_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub active_workspace_id: String,
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayout {
    pub workspace_id: String,
    pub sidebar_collapsed: bool,
    pub active_tab_id: String,
    pub tabs: Vec<WorkspaceLayoutTab>,
    pub selected_api_request_id: Option<String>,
    pub selected_database_connection_id: Option<String>,
    pub selected_ssh_connection_id: Option<String>,
    #[serde(default)]
    pub sidebar_width: i32,
    #[serde(default)]
    pub bottom_panel_height: i32,
    #[serde(default)]
    pub right_inspector_width: i32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayoutTab {
    pub id: String,
    pub title: String,
    pub kind: String,
}
