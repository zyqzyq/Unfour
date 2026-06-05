use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppCommand {
    Api(ApiCommand),
    Database(DatabaseCommand),
    Ssh(SshCommand),
    Workspace(WorkspaceCommand),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiCommand {
    SendRequest,
    SaveRequest,
    ListHistory,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DatabaseCommand {
    TestConnection,
    ListSchema,
    ExecuteSql,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SshCommand {
    Connect,
    WriteInput,
    ResizePty,
    Close,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceCommand {
    Create,
    Rename,
    Delete,
    SetActive,
}

pub fn capability_ids() -> Vec<String> {
    [
        "workspace.create",
        "workspace.rename",
        "api.send_request",
        "api.save_request",
        "ssh.connect.reserved",
        "database.execute_sql.reserved",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
