use crate::{mcp_client_config, AppDistribution, AppState};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use unfour_core::{AppError, AppResult};

pub use crate::mcp_client_config::{McpClient, McpClientStatus, McpClientStatusResult};

/// Whether the running app is a debug/dev build or a release/installed build.
///
/// Surfaced to the UI so it can tailor the "binary not found" guidance:
/// dev builds tell the user how to compile the sidecar, release builds tell
/// them to reinstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpBuildKind {
    Dev,
    Release,
}

/// Resolved location of the `unfour-mcp` sidecar binary for external MCP
/// clients (Codex/Claude/Cursor). The path is dynamic: it points at wherever
/// the current executable (and its bundled resources) actually live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpBinaryPathResult {
    /// Command or absolute path the external MCP client should invoke.
    pub path: String,
    /// Whether the command is available for the current build.
    pub found: bool,
    /// Build kind, so the UI can tailor its guidance.
    pub build_kind: McpBuildKind,
}

#[tauri::command]
pub fn mcp_binary_path(state: State<'_, AppState>) -> AppResult<McpBinaryPathResult> {
    Ok(current_mcp_binary_path(&state))
}

#[tauri::command]
pub fn mcp_client_status(
    client: McpClient,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<McpClientStatusResult> {
    let home = app
        .path()
        .home_dir()
        .map_err(|_| AppError::Config("The user home directory is not available.".to_string()))?;
    let binary = current_mcp_binary_path(&state);
    Ok(mcp_client_config::status(&home, client, &binary.path))
}

#[tauri::command]
pub fn mcp_client_configure(
    client: McpClient,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<McpClientStatusResult> {
    let home = app
        .path()
        .home_dir()
        .map_err(|_| AppError::Config("The user home directory is not available.".to_string()))?;
    let binary = current_mcp_binary_path(&state);
    mcp_client_config::configure(&home, client, &binary.path, binary.found)
}

fn current_mcp_binary_path(state: &AppState) -> McpBinaryPathResult {
    let build_kind = if cfg!(debug_assertions) {
        McpBuildKind::Dev
    } else {
        McpBuildKind::Release
    };

    resolve_mcp_binary_path(build_kind, state.config.distribution)
}

/// Plain runnable name (no target triple), used for the dev `target/debug`
/// sibling and the intuitive "next to the app" release layout.
fn binary_name() -> String {
    if cfg!(windows) {
        "unfour-mcp.exe".to_string()
    } else {
        "unfour-mcp".to_string()
    }
}

/// Tauri `externalBin` name, which carries the full target triple.
fn sidecar_name() -> String {
    let ext = if cfg!(windows) { ".exe" } else { "" };
    format!("unfour-mcp-{}{}", target_triple(), ext)
}

#[allow(clippy::needless_return)]
fn target_triple() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "aarch64-pc-windows-msvc";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-gnu";
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    return "unknown";
}

fn current_exe_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.to_path_buf())
}

fn resolve_mcp_binary_path(
    build_kind: McpBuildKind,
    distribution: AppDistribution,
) -> McpBinaryPathResult {
    // Microsoft Store registers this executable through the package manifest's
    // windows.appExecutionAlias. The physical WindowsApps directory contains
    // versioned package folders and must never be copied into client config.
    if distribution == AppDistribution::MicrosoftStore {
        return McpBinaryPathResult {
            path: binary_name(),
            found: true,
            build_kind,
        };
    }

    let recommended = current_exe_dir()
        .map(|dir| dir.join(binary_name()))
        .unwrap_or_else(|| std::path::PathBuf::from(binary_name()));

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = current_exe_dir() {
        // Dev build sibling and the intuitive "next to the app" release layout.
        candidates.push(dir.join(binary_name()));
        // Tauri v2 sidecar bundling: <app>/resources/bin/<name>-<triple>[.exe].
        candidates.push(dir.join("resources").join("bin").join(sidecar_name()));
        // macOS: <app>/../Resources/<name>-<triple>.
        candidates.push(dir.join("..").join("Resources").join(sidecar_name()));
        // Dev `tauri dev` prepared externalBin: <target>/<profile>/../../src-tauri/binaries.
        candidates.push(
            dir.join("..")
                .join("..")
                .join("src-tauri")
                .join("binaries")
                .join(sidecar_name()),
        );
    }

    for candidate in &candidates {
        if candidate.is_file() {
            return McpBinaryPathResult {
                path: candidate.to_string_lossy().to_string(),
                found: true,
                build_kind,
            };
        }
    }

    McpBinaryPathResult {
        path: recommended.to_string_lossy().to_string(),
        found: false,
        build_kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_store_uses_the_stable_execution_alias() {
        let result =
            resolve_mcp_binary_path(McpBuildKind::Release, AppDistribution::MicrosoftStore);

        assert_eq!(result.path, binary_name());
        assert!(result.found);
    }
}
