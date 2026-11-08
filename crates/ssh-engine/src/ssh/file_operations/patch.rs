#[cfg(any(feature = "ssh-native", test))]
use unfour_core::models::{SshFilePatchInput, SSH_PATCH_MAX_FILE_BYTES};

#[cfg(any(feature = "ssh-native", test))]
pub(super) fn command() -> String {
    // All arguments, including the path, travel over stdin. No `--`/argv offsets.
    let script = serde_json::to_string(include_str!("patch.py")).expect("fixed Python script");
    let expression = format!("exec({script})");
    format!("python3 -c '{}'", expression.replace('\'', "'\\''"))
}

#[cfg(any(feature = "ssh-native", test))]
pub(super) fn payload(input: &SshFilePatchInput) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "path": input.path,
        "search": input.search,
        "replace": input.replace,
        "allowMultiple": input.allow_multiple,
        "maxFileBytes": SSH_PATCH_MAX_FILE_BYTES,
    }))
    .expect("file patch payload")
}
