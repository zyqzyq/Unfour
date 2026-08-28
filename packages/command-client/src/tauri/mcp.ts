import { call } from "./invoke";

export type McpBuildKind = "dev" | "release";

export interface McpBinaryPathResult {
  /** Command or absolute path the external MCP client should invoke. */
  path: string;
  /** Whether the command is available for the current build. */
  found: boolean;
  /** Build kind, so the UI can tailor its guidance. */
  buildKind: McpBuildKind;
}

export type McpClient = "codex" | "cursor";
export type McpClientStatus = "notConfigured" | "configured" | "outdated" | "error";

export interface McpClientStatusResult {
  client: McpClient;
  status: McpClientStatus;
  configPath: string;
}

export function getMcpBinaryPath() {
  return call<McpBinaryPathResult>("mcp_binary_path");
}

export function getMcpClientStatus(client: McpClient) {
  return call<McpClientStatusResult>("mcp_client_status", { client });
}

export function configureMcpClient(client: McpClient) {
  return call<McpClientStatusResult>("mcp_client_configure", { client });
}
