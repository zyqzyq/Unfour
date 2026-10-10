import type { Workspace, WorkspaceMcpPolicy } from "@unfour/command-client";

/** Display only. Rust remains the authority for MCP tool authorization. */
export function resolveWorkspaceMcpPolicy(
  workspace: Pick<Workspace, "environmentType" | "mcpPolicy">,
): Exclude<WorkspaceMcpPolicy, "auto"> {
  if (workspace.mcpPolicy !== "auto") return workspace.mcpPolicy;
  if (workspace.environmentType === "dev") return "full_access";
  if (workspace.environmentType === "test") return "guarded";
  return "read_only";
}
