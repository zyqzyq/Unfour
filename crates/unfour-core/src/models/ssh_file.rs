use serde::{Deserialize, Serialize};

/// Inline file data is bounded independently of shell command length.
pub const SSH_FILE_MAX_CONTENT_BYTES: usize = 4 * 1024 * 1024;
pub const SSH_PATCH_MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SshFileWriteMode {
    Overwrite,
    Append,
    Create,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshFileWriteInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub path: String,
    pub content: String,
    pub mode: SshFileWriteMode,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshFilePatchInput {
    pub workspace_id: String,
    pub connection_id: String,
    pub path: String,
    pub search: String,
    pub replace: String,
    pub allow_multiple: bool,
    pub timeout_ms: Option<u64>,
}
