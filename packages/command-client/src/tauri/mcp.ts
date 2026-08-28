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

export function getMcpBinaryPath() {
  return call<McpBinaryPathResult>("mcp_binary_path");
}
