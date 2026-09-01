use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub active_workspace_id: String,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceVariable {
    pub id: String,
    pub workspace_id: String,
    pub key: String,
    pub value: String,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
}

impl fmt::Debug for WorkspaceVariable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceVariable")
            .field("id", &self.id)
            .field("workspace_id", &self.workspace_id)
            .field("key", &self.key)
            .field("value", &"[REDACTED]")
            .field("is_secret", &self.is_secret)
            .field("is_enabled", &self.is_enabled)
            .field("description", &self.description)
            .field("sort_order", &self.sort_order)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_at", &self.deleted_at)
            .field("revision", &self.revision)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, FromRow, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEnvironmentVariable {
    pub id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub key: String,
    pub value: String,
    pub is_secret: bool,
    pub is_enabled: bool,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEnvironment {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub variables: Vec<WorkspaceEnvironmentVariable>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub revision: i64,
}

impl fmt::Debug for WorkspaceEnvironmentVariable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceEnvironmentVariable")
            .field("id", &self.id)
            .field("workspace_id", &self.workspace_id)
            .field("environment_id", &self.environment_id)
            .field("key", &self.key)
            .field("value", &"[REDACTED]")
            .field("is_secret", &self.is_secret)
            .field("is_enabled", &self.is_enabled)
            .field("description", &self.description)
            .field("sort_order", &self.sort_order)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_at", &self.deleted_at)
            .field("revision", &self.revision)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceVariableInput {
    #[serde(default)]
    pub id: Option<String>,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default = "default_enabled")]
    pub is_enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

impl fmt::Debug for WorkspaceVariableInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceVariableInput")
            .field("id", &self.id)
            .field("key", &self.key)
            .field("value", &"[REDACTED]")
            .field("is_secret", &self.is_secret)
            .field("is_enabled", &self.is_enabled)
            .field("description", &self.description)
            .field("sort_order", &self.sort_order)
            .finish()
    }
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSidebarWidths {
    pub api: i32,
    pub ssh: i32,
    pub database: i32,
}

impl Default for WorkspaceSidebarWidths {
    fn default() -> Self {
        Self {
            api: 320,
            ssh: 248,
            database: 280,
        }
    }
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
    pub sidebar_widths: WorkspaceSidebarWidths,
    /// Accepted only when decoding payloads from the previous global-width API.
    /// It is never serialized back; layout snapshots use `sidebar_widths`.
    #[serde(default, skip_serializing)]
    pub sidebar_width: Option<i32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_debug_output_redacts_values() {
        let input = WorkspaceVariableInput {
            id: Some("variable-1".to_string()),
            key: "TOKEN".to_string(),
            value: "top-secret".to_string(),
            is_secret: true,
            is_enabled: true,
            description: None,
            sort_order: 0,
        };

        let debug = format!("{input:?}");
        assert!(!debug.contains("top-secret"));
        assert!(debug.contains("REDACTED"));

        let variable = WorkspaceVariable {
            id: "variable-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            key: input.key,
            value: input.value,
            is_secret: input.is_secret,
            is_enabled: input.is_enabled,
            description: input.description,
            sort_order: input.sort_order,
            created_at: "2026-07-23T00:00:00Z".to_string(),
            updated_at: "2026-07-23T00:00:00Z".to_string(),
            deleted_at: None,
            revision: 1,
        };
        assert!(!format!("{variable:?}").contains("top-secret"));
    }
}
