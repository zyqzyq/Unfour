import type { McpClient } from "../mcp";
import { UNHANDLED, type MockResult } from "./types";

/**
 * Browser preview mock. We return `found: false` with a `dev` build kind so the
 * Settings → MCP "binary not found" guidance (including the dev-specific hint)
 * is visible without a real Tauri backend.
 */
export function handleMcpMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "mcp_binary_path") {
    return {
      path: "/mock/unfour-mcp",
      found: false,
      buildKind: "dev",
    } as T;
  }

  if (command === "mcp_client_status") {
    const client = String(args?.client ?? "codex") as McpClient;
    return {
      client,
      status: "notConfigured",
      configPath: client === "codex" ? "/mock/.codex/config.toml" : "/mock/.cursor/mcp.json",
    } as T;
  }

  if (command === "mcp_client_configure") {
    throw new Error("The Unfour MCP binary is not available; configuration was not changed.");
  }

  return UNHANDLED;
}
