use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{value, Array, DocumentMut, Item, Table};
use unfour_core::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum McpClient {
    Codex,
    Cursor,
}

impl McpClient {
    fn config_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Codex => home.join(".codex").join("config.toml"),
            Self::Cursor => home.join(".cursor").join("mcp.json"),
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum McpClientStatus {
    NotConfigured,
    Configured,
    Outdated,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientStatusResult {
    pub client: McpClient,
    pub status: McpClientStatus,
    pub config_path: String,
}

pub fn status(home: &Path, client: McpClient, command: &str) -> McpClientStatusResult {
    let config_path = client.config_path(home);
    let status = inspect_config(&config_path, client, command).unwrap_or(McpClientStatus::Error);
    status_result(client, status, config_path)
}

pub fn configure(
    home: &Path,
    client: McpClient,
    command: &str,
    binary_found: bool,
) -> AppResult<McpClientStatusResult> {
    if !binary_found {
        return Err(AppError::Config(
            "The Unfour MCP binary is not available; configuration was not changed.".to_string(),
        ));
    }

    let config_path = client.config_path(home);
    let content = match client {
        McpClient::Codex => merge_codex_config(&config_path, command)?,
        McpClient::Cursor => merge_cursor_config(&config_path, command)?,
    };
    write_config(&config_path, content.as_bytes(), client)?;

    Ok(status_result(
        client,
        McpClientStatus::Configured,
        config_path,
    ))
}

fn status_result(
    client: McpClient,
    status: McpClientStatus,
    config_path: PathBuf,
) -> McpClientStatusResult {
    McpClientStatusResult {
        client,
        status,
        config_path: config_path.to_string_lossy().to_string(),
    }
}

fn inspect_config(path: &Path, client: McpClient, command: &str) -> AppResult<McpClientStatus> {
    let Some(content) = read_optional_config(path, client)? else {
        return Ok(McpClientStatus::NotConfigured);
    };
    match client {
        McpClient::Codex => inspect_codex_config(&content, command),
        McpClient::Cursor => inspect_cursor_config(&content, command),
    }
}

fn inspect_codex_config(content: &str, command: &str) -> AppResult<McpClientStatus> {
    let document = parse_codex(content)?;
    let Some(servers) = document.get("mcp_servers").and_then(Item::as_table_like) else {
        return if document.contains_key("mcp_servers") {
            Err(invalid_structure("Codex", "mcp_servers must be a table"))
        } else {
            Ok(McpClientStatus::NotConfigured)
        };
    };
    let Some(unfour) = servers.get("unfour") else {
        return Ok(McpClientStatus::NotConfigured);
    };
    let Some(unfour) = unfour.as_table_like() else {
        return Ok(McpClientStatus::Outdated);
    };

    let command_matches = unfour.get("command").and_then(Item::as_str) == Some(command);
    let args_match = unfour
        .get("args")
        .and_then(Item::as_array)
        .is_some_and(Array::is_empty);
    Ok(if command_matches && args_match {
        McpClientStatus::Configured
    } else {
        McpClientStatus::Outdated
    })
}

fn inspect_cursor_config(content: &str, command: &str) -> AppResult<McpClientStatus> {
    let document = parse_cursor(content)?;
    let Some(root) = document.as_object() else {
        return Err(invalid_structure(
            "Cursor",
            "the root value must be an object",
        ));
    };
    let Some(servers_value) = root.get("mcpServers") else {
        return Ok(McpClientStatus::NotConfigured);
    };
    let Some(servers) = servers_value.as_object() else {
        return Err(invalid_structure("Cursor", "mcpServers must be an object"));
    };
    let Some(unfour) = servers.get("unfour") else {
        return Ok(McpClientStatus::NotConfigured);
    };
    let Some(unfour) = unfour.as_object() else {
        return Ok(McpClientStatus::Outdated);
    };

    Ok(if cursor_entry_matches(unfour, command) {
        McpClientStatus::Configured
    } else {
        McpClientStatus::Outdated
    })
}

fn merge_codex_config(path: &Path, command: &str) -> AppResult<String> {
    let content = read_optional_config(path, McpClient::Codex)?.unwrap_or_default();
    let mut document = parse_codex(&content)?;

    if !document.contains_key("mcp_servers") {
        document.insert("mcp_servers", Item::Table(Table::new()));
    }
    let servers = document
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| invalid_structure("Codex", "mcp_servers must be a table"))?;

    if !servers.contains_key("unfour") {
        servers.insert("unfour", Item::Table(Table::new()));
    }
    let unfour_item = servers
        .get_mut("unfour")
        .expect("unfour table was inserted above");
    if !unfour_item.is_table_like() {
        *unfour_item = Item::Table(Table::new());
    }
    let unfour = unfour_item
        .as_table_like_mut()
        .expect("unfour item was normalized to a table");
    unfour.insert("command", value(command));
    unfour.insert("args", value(Array::new()));

    Ok(document.to_string())
}

fn merge_cursor_config(path: &Path, command: &str) -> AppResult<String> {
    let content = read_optional_config(path, McpClient::Cursor)?;
    let mut document = match content {
        Some(content) => parse_cursor(&content)?,
        None => JsonValue::Object(JsonMap::new()),
    };
    let root = document
        .as_object_mut()
        .ok_or_else(|| invalid_structure("Cursor", "the root value must be an object"))?;
    if !root.contains_key("mcpServers") {
        root.insert("mcpServers".to_string(), JsonValue::Object(JsonMap::new()));
    }
    let servers = root
        .get_mut("mcpServers")
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| invalid_structure("Cursor", "mcpServers must be an object"))?;
    let unfour = servers
        .entry("unfour".to_string())
        .or_insert_with(|| JsonValue::Object(JsonMap::new()));
    if !unfour.is_object() {
        *unfour = JsonValue::Object(JsonMap::new());
    }
    let unfour = unfour
        .as_object_mut()
        .expect("unfour value was normalized to an object");
    let (launch_command, launch_args) = cursor_launch_spec(command);
    unfour.insert("command".to_string(), JsonValue::String(launch_command));
    unfour.insert("args".to_string(), JsonValue::Array(launch_args));

    let mut output = serde_json::to_string_pretty(&document)?;
    output.push('\n');
    Ok(output)
}

/// Cursor still launches stdio MCP servers through `cmd.exe` on Windows.
/// An unquoted path with whitespace is split, so `D:\Program Files\...`
/// becomes the command `D:\Program` and the connection closes immediately.
fn cursor_launch_spec(command: &str) -> (String, Vec<JsonValue>) {
    if cursor_windows_cmd_wrapper_required(command) {
        (
            "cmd.exe".to_string(),
            vec![
                JsonValue::String("/c".to_string()),
                JsonValue::String(command.to_string()),
            ],
        )
    } else {
        (command.to_string(), Vec::new())
    }
}

fn cursor_windows_cmd_wrapper_required(command: &str) -> bool {
    cfg!(windows) && command_has_whitespace(command)
}

fn command_has_whitespace(command: &str) -> bool {
    command.chars().any(char::is_whitespace)
}

fn cursor_entry_matches(unfour: &JsonMap<String, JsonValue>, command: &str) -> bool {
    let Some(configured_command) = unfour.get("command").and_then(JsonValue::as_str) else {
        return false;
    };
    let configured_args = unfour
        .get("args")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let (expected_command, expected_args) = cursor_launch_spec(command);
    configured_command == expected_command && configured_args == expected_args.as_slice()
}

fn parse_codex(content: &str) -> AppResult<DocumentMut> {
    content.parse::<DocumentMut>().map_err(|_| {
        AppError::Config("The existing Codex configuration is not valid TOML.".to_string())
    })
}

fn parse_cursor(content: &str) -> AppResult<JsonValue> {
    // Some Windows editors write UTF-8 JSON with a BOM. Cursor accepts that
    // file, so treat the BOM as an encoding marker rather than invalid JSON.
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    serde_json::from_str(content).map_err(|_| {
        AppError::Config("The existing Cursor configuration is not valid JSON.".to_string())
    })
}

fn invalid_structure(client: &str, detail: &str) -> AppError {
    AppError::Config(format!(
        "The existing {client} configuration cannot be merged safely: {detail}."
    ))
}

fn read_optional_config(path: &Path, client: McpClient) -> AppResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Config(format!(
            "Could not read the {} configuration: {error}",
            client.display_name()
        ))),
    }
}

fn write_config(path: &Path, content: &[u8], client: McpClient) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Config(format!(
            "The {} configuration path has no parent directory.",
            client.display_name()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| write_error(client, error))?;

    let (temp_path, mut temp_file) =
        create_temp_file(path).map_err(|error| write_error(client, error))?;
    let write_result = (|| -> io::Result<()> {
        temp_file.write_all(content)?;
        temp_file.sync_all()?;
        drop(temp_file);

        if path.exists() {
            let backup = backup_path(path);
            fs::copy(path, &backup)?;
            OpenOptions::new().write(true).open(&backup)?.sync_all()?;
        }

        replace_file(&temp_path, path)?;
        sync_parent_directory(parent);
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(write_error(client, error));
    }
    Ok(())
}

fn create_temp_file(path: &Path) -> io::Result<(PathBuf, File)> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mcp-config");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..16 {
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.{}.tmp",
            std::process::id(),
            nonce,
            attempt
        ));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique temporary configuration file",
    ))
}

fn backup_path(path: &Path) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| OsString::from("mcp-config"));
    file_name.push(".bak");
    path.with_file_name(file_name)
}

fn write_error(client: McpClient, error: io::Error) -> AppError {
    AppError::Config(format!(
        "Could not update the {} configuration: {error}",
        client.display_name()
    ))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
    }

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        if destination.exists() {
            ReplaceFileW(
                destination_wide.as_ptr(),
                source_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } else {
            MoveFileExW(
                source_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if result != 0 {
        return Ok(());
    }

    // Some packaged or policy-restricted Windows environments reject the
    // atomic replacement APIs even when both files are writable. A sibling
    // `.bak` has already been synced before this point, so fall back to a
    // remove-and-rename replacement and restore that backup if the rename
    // itself fails.
    if !destination.exists() {
        return Err(io::Error::last_os_error());
    }
    fs::remove_file(destination)?;
    if let Err(error) = fs::rename(source, destination) {
        let backup = backup_path(destination);
        if backup.is_file() {
            let _ = fs::copy(backup, destination);
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) {
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_COMMAND: &str = r"D:\Apps\Unfour\unfour-mcp.exe";
    const OLD_COMMAND: &str = r"D:\Old\Unfour\unfour-mcp.exe";
    const SPACED_COMMAND: &str = r"D:\Program Files\Unfour\unfour-mcp.exe";

    struct TestHome {
        path: PathBuf,
    }

    impl TestHome {
        fn new(name: &str) -> Self {
            let path = std::env::current_dir()
                .expect("test working directory")
                .join("target")
                .join("test-tmp")
                .join(format!(
                    "unfour-mcp-client-{name}-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn configure_test(home: &TestHome, client: McpClient) -> McpClientStatusResult {
        configure(home.path(), client, CURRENT_COMMAND, true).expect("configure MCP client")
    }

    #[test]
    fn codex_missing_config_is_created() {
        let home = TestHome::new("codex-create");

        let result = configure_test(&home, McpClient::Codex);

        assert_eq!(result.status, McpClientStatus::Configured);
        let content = fs::read_to_string(home.path().join(".codex/config.toml")).unwrap();
        let document = parse_codex(&content).unwrap();
        assert_eq!(
            document["mcp_servers"]["unfour"]["command"].as_str(),
            Some(CURRENT_COMMAND)
        );
        assert!(document["mcp_servers"]["unfour"]["args"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn codex_preserves_other_configuration_and_comments() {
        let home = TestHome::new("codex-preserve-general");
        let path = home.path().join(".codex/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "# keep this comment\nmodel = \"gpt-5\"\nmodel_reasoning_effort = \"high\"\n",
        )
        .unwrap();

        configure_test(&home, McpClient::Codex);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# keep this comment"));
        assert!(content.contains("model = \"gpt-5\""));
        assert!(content.contains("model_reasoning_effort = \"high\""));
    }

    #[test]
    fn codex_preserves_other_mcp_servers() {
        let home = TestHome::new("codex-preserve-servers");
        let path = home.path().join(".codex/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "[mcp_servers.example]\ncommand = \"example-mcp\"\nargs = [\"serve\"]\n",
        )
        .unwrap();

        configure_test(&home, McpClient::Codex);

        let content = fs::read_to_string(&path).unwrap();
        let document = parse_codex(&content).unwrap();
        assert_eq!(
            document["mcp_servers"]["example"]["command"].as_str(),
            Some("example-mcp")
        );
        assert_eq!(
            document["mcp_servers"]["example"]["args"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn codex_outdated_command_is_updated_and_backed_up() {
        let home = TestHome::new("codex-update");
        let path = home.path().join(".codex/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!("[mcp_servers.unfour]\ncommand = {OLD_COMMAND:?}\nargs = []\nkeep = true\n"),
        )
        .unwrap();

        assert_eq!(
            status(home.path(), McpClient::Codex, CURRENT_COMMAND).status,
            McpClientStatus::Outdated
        );
        configure_test(&home, McpClient::Codex);

        let content = fs::read_to_string(&path).unwrap();
        let document = parse_codex(&content).unwrap();
        assert_eq!(
            document["mcp_servers"]["unfour"]["command"].as_str(),
            Some(CURRENT_COMMAND)
        );
        assert_eq!(
            document["mcp_servers"]["unfour"]["keep"].as_bool(),
            Some(true)
        );
        let backup = fs::read_to_string(backup_path(&path)).unwrap();
        let backup_document = parse_codex(&backup).unwrap();
        assert_eq!(
            backup_document["mcp_servers"]["unfour"]["command"].as_str(),
            Some(OLD_COMMAND)
        );
    }

    #[test]
    fn cursor_missing_config_is_created() {
        let home = TestHome::new("cursor-create");

        let result = configure_test(&home, McpClient::Cursor);

        assert_eq!(result.status, McpClientStatus::Configured);
        let content = fs::read_to_string(home.path().join(".cursor/mcp.json")).unwrap();
        let document = parse_cursor(&content).unwrap();
        assert_eq!(
            document["mcpServers"]["unfour"]["command"].as_str(),
            Some(CURRENT_COMMAND)
        );
        assert_eq!(
            document["mcpServers"]["unfour"]["args"],
            JsonValue::Array(Vec::new())
        );
    }

    #[test]
    fn cursor_preserves_other_settings_servers_and_unfour_fields() {
        let home = TestHome::new("cursor-preserve");
        let path = home.path().join(".cursor/mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                r#"{{
  "theme": "dark",
  "mcpServers": {{
    "example": {{ "command": "example-mcp", "args": ["serve"] }},
    "unfour": {{ "command": {OLD_COMMAND:?}, "args": [], "disabled": false }}
  }}
}}"#
            ),
        )
        .unwrap();

        configure_test(&home, McpClient::Cursor);

        let content = fs::read_to_string(&path).unwrap();
        let document = parse_cursor(&content).unwrap();
        assert_eq!(document["theme"], "dark");
        assert_eq!(document["mcpServers"]["example"]["command"], "example-mcp");
        assert_eq!(document["mcpServers"]["unfour"]["command"], CURRENT_COMMAND);
        assert_eq!(document["mcpServers"]["unfour"]["disabled"], false);
    }

    #[test]
    fn cursor_outdated_command_is_reported_and_updated() {
        let home = TestHome::new("cursor-update");
        let path = home.path().join(".cursor/mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(r#"{{"mcpServers":{{"unfour":{{"command":{OLD_COMMAND:?},"args":[]}}}}}}"#),
        )
        .unwrap();

        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Outdated
        );
        configure_test(&home, McpClient::Cursor);
        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Configured
        );
    }

    #[cfg(windows)]
    #[test]
    fn cursor_windows_path_with_spaces_uses_cmd_wrapper() {
        let home = TestHome::new("cursor-space-wrapper");

        let result = configure(home.path(), McpClient::Cursor, SPACED_COMMAND, true)
            .expect("configure MCP client");

        assert_eq!(result.status, McpClientStatus::Configured);
        let content = fs::read_to_string(home.path().join(".cursor/mcp.json")).unwrap();
        let document = parse_cursor(&content).unwrap();
        assert_eq!(document["mcpServers"]["unfour"]["command"], "cmd.exe");
        assert_eq!(
            document["mcpServers"]["unfour"]["args"],
            JsonValue::Array(vec![
                JsonValue::String("/c".to_string()),
                JsonValue::String(SPACED_COMMAND.to_string()),
            ])
        );
        assert_eq!(
            status(home.path(), McpClient::Cursor, SPACED_COMMAND).status,
            McpClientStatus::Configured
        );
    }

    #[cfg(windows)]
    #[test]
    fn cursor_windows_direct_path_with_spaces_is_outdated_and_rewritten() {
        let home = TestHome::new("cursor-space-direct-outdated");
        let path = home.path().join(".cursor/mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(r#"{{"mcpServers":{{"unfour":{{"command":{SPACED_COMMAND:?},"args":[]}}}}}}"#),
        )
        .unwrap();

        assert_eq!(
            status(home.path(), McpClient::Cursor, SPACED_COMMAND).status,
            McpClientStatus::Outdated
        );

        configure(home.path(), McpClient::Cursor, SPACED_COMMAND, true)
            .expect("configure MCP client");

        assert_eq!(
            status(home.path(), McpClient::Cursor, SPACED_COMMAND).status,
            McpClientStatus::Configured
        );
        let document = parse_cursor(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(document["mcpServers"]["unfour"]["command"], "cmd.exe");
    }

    #[cfg(not(windows))]
    #[test]
    fn cursor_non_windows_keeps_path_with_spaces_in_command() {
        let home = TestHome::new("cursor-space-unix");

        configure(home.path(), McpClient::Cursor, SPACED_COMMAND, true)
            .expect("configure MCP client");

        let document =
            parse_cursor(&fs::read_to_string(home.path().join(".cursor/mcp.json")).unwrap())
                .unwrap();
        assert_eq!(
            document["mcpServers"]["unfour"]["command"].as_str(),
            Some(SPACED_COMMAND)
        );
        assert_eq!(
            document["mcpServers"]["unfour"]["args"],
            JsonValue::Array(Vec::new())
        );
    }

    #[test]
    fn cursor_utf8_bom_and_legacy_space_workaround_are_updated() {
        let home = TestHome::new("cursor-bom-legacy-command");
        let path = home.path().join(".cursor/mcp.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "\u{feff}{\"mcpServers\":{\"unfour\":{\"command\":\"cmd\",\"args\":[\"/c\",\"D:\\\\Program Files\\\\Unfour\\\\unfour-mcp.exe\"]}}}",
        )
        .unwrap();

        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Outdated
        );

        configure_test(&home, McpClient::Cursor);

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.starts_with('\u{feff}'));
        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Configured
        );
    }

    #[test]
    fn correctly_configured_clients_ignore_extra_fields() {
        let home = TestHome::new("configured-status");
        configure_test(&home, McpClient::Codex);
        configure_test(&home, McpClient::Cursor);

        assert_eq!(
            status(home.path(), McpClient::Codex, CURRENT_COMMAND).status,
            McpClientStatus::Configured
        );
        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Configured
        );
    }

    #[test]
    fn missing_binary_prevents_any_configuration_write() {
        let home = TestHome::new("missing-binary");

        let error = configure(home.path(), McpClient::Codex, CURRENT_COMMAND, false).unwrap_err();

        assert_eq!(error.code(), "CONFIG_ERROR");
        assert!(!home.path().join(".codex/config.toml").exists());
    }

    #[test]
    fn malformed_existing_files_report_error_status_without_overwriting() {
        let home = TestHome::new("malformed");
        let codex_path = home.path().join(".codex/config.toml");
        let cursor_path = home.path().join(".cursor/mcp.json");
        fs::create_dir_all(codex_path.parent().unwrap()).unwrap();
        fs::create_dir_all(cursor_path.parent().unwrap()).unwrap();
        fs::write(&codex_path, "this = [is not valid").unwrap();
        fs::write(&cursor_path, "{not-json}").unwrap();

        assert_eq!(
            status(home.path(), McpClient::Codex, CURRENT_COMMAND).status,
            McpClientStatus::Error
        );
        assert_eq!(
            status(home.path(), McpClient::Cursor, CURRENT_COMMAND).status,
            McpClientStatus::Error
        );
        assert!(configure(home.path(), McpClient::Codex, CURRENT_COMMAND, true).is_err());
        assert!(configure(home.path(), McpClient::Cursor, CURRENT_COMMAND, true).is_err());
        assert_eq!(
            fs::read_to_string(codex_path).unwrap(),
            "this = [is not valid"
        );
        assert_eq!(fs::read_to_string(cursor_path).unwrap(), "{not-json}");
    }
}
