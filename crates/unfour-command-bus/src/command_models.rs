use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    All,
    Api,
    Database,
    Redis,
    Ssh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadCommand {
    CurrentWorkspace,
    ListWorkspaces,
    ListConnections {
        connection_type: ConnectionType,
    },
    ListConnectionsInWorkspace {
        workspace_id: String,
        connection_type: ConnectionType,
    },
    ApiListCollections {
        workspace_id: Option<String>,
    },
    ApiListRequests {
        workspace_id: Option<String>,
        collection_id: Option<String>,
    },
    ApiGetRequest {
        request_id: String,
    },
    ApiListHistory {
        workspace_id: Option<String>,
        limit: Option<i64>,
    },
    ApiGetHistory {
        workspace_id: Option<String>,
        history_id: String,
    },
    ApiListEnvironments {
        workspace_id: Option<String>,
    },
    ListActivity {
        workspace_id: Option<String>,
        limit: Option<i64>,
    },
}

#[derive(Debug, Clone)]
pub enum ReadCommandResult {
    CurrentWorkspace(CurrentWorkspaceResult),
    Workspaces(WorkspaceListResult),
    Connections(ConnectionListResult),
    ApiCollections(ApiCollectionListResult),
    ApiRequests(ApiRequestListResult),
    ApiRequest(ApiRequestDetailResult),
    ApiHistory(ApiHistoryListResult),
    ApiHistoryDetailResult(ApiHistoryDetailResult),
    ApiEnvironments(ApiEnvironmentListResult),
    Activity(ActivityListResult),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWorkspaceResult {
    pub workspace_id: String,
    pub workspace_name: String,
    pub environment_type: String,
    pub mcp_policy: String,
    pub workspace_root: Option<String>,
    pub mode: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResult {
    pub workspaces: Vec<WorkspaceSummary>,
    pub active_workspace_id: String,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_active: bool,
    pub environment_type: String,
    pub mcp_policy: String,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionListResult {
    pub connections: Vec<SafeConnection>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeConnection {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub connection_type: String,
    pub workspace_id: String,
    pub safe_summary: SafeConnectionSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeConnectionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionListResult {
    pub collections: Vec<ApiCollectionSummary>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCollectionSummary {
    pub id: String,
    pub name: String,
    pub request_count: usize,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestListResult {
    pub requests: Vec<ApiRequestSummary>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestSummary {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url_preview: String,
    pub collection_id: String,
    pub workspace_id: String,
    pub has_body: bool,
    pub header_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequestDetailResult {
    pub request: ApiSavedRequest,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryListResult {
    pub history: Vec<ApiHistoryItem>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiHistoryDetailResult {
    pub detail: ApiHistoryDetail,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvironmentListResult {
    pub environments: Vec<ApiEnvironment>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListResult {
    pub activity: Vec<ActivityItem>,
    pub count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub workspace_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    /// Redacted summary payload recorded with the event. Consumers that surface
    /// this to an LLM apply an additional masking pass as defense-in-depth.
    pub details: serde_json::Value,
    pub created_at: String,
}
