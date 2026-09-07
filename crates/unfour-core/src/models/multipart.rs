use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};

pub const MULTIPART_BODY_KIND: &str = "multipart-form-data";

/// Persistable definition. Unknown fields (including paths and bytes) are rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum ApiMultipartPart {
    Text {
        id: String,
        enabled: bool,
        key: String,
        value: String,
    },
    File {
        id: String,
        enabled: bool,
        key: String,
        #[serde(rename = "fileName")]
        file_name: Option<String>,
    },
}

impl ApiMultipartPart {
    pub fn id(&self) -> &str {
        match self {
            Self::Text { id, .. } | Self::File { id, .. } => id,
        }
    }
    pub fn enabled(&self) -> bool {
        match self {
            Self::Text { enabled, .. } | Self::File { enabled, .. } => *enabled,
        }
    }
    pub fn key(&self) -> &str {
        match self {
            Self::Text { key, .. } | Self::File { key, .. } => key,
        }
    }
}

/// Desktop-only binding, joined to the definition by stable id. Never serialized.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiMultipartRuntimePart {
    pub id: String,
    pub file_path: String,
}
impl std::fmt::Debug for ApiMultipartRuntimePart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiMultipartRuntimePart(<local binding>)")
    }
}

pub fn parse_multipart_definition(body: Option<&str>) -> AppResult<Vec<ApiMultipartPart>> {
    let parts: Vec<ApiMultipartPart> = serde_json::from_str(body.unwrap_or("[]"))
        .map_err(|_| AppError::Validation("Invalid multipart definition".into()))?;
    let mut ids = std::collections::HashSet::new();
    for part in &parts {
        if part.id().trim().is_empty() || !ids.insert(part.id()) {
            return Err(AppError::Validation(
                "Multipart part ids must be unique and non-empty".into(),
            ));
        }
        if let ApiMultipartPart::File {
            file_name: Some(name),
            ..
        } = part
        {
            if name.contains(['/', '\\', ':', '\r', '\n']) {
                return Err(AppError::Validation(
                    "Multipart filename must be a basename".into(),
                ));
            }
        }
    }
    Ok(parts)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPickedFile {
    pub path: String,
    pub name: String,
}
