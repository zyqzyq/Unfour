import { describe, expect, it } from "vitest";
import type { WorkspaceEnvironmentType, WorkspaceMcpPolicy } from "@unfour/command-client";
import { resolveWorkspaceMcpPolicy } from "./workspace-mcp-policy";

describe("Workspace MCP display policy", () => {
  it.each([
    ["dev", "full_access"], ["test", "guarded"], ["prod", "read_only"],
  ] as const)("resolves Auto for %s to %s", (environmentType, effective) => {
    expect(resolveWorkspaceMcpPolicy({ environmentType, mcpPolicy: "auto" })).toBe(effective);
  });

  it.each(["disabled", "read_only", "guarded", "full_access"] as WorkspaceMcpPolicy[])(
    "preserves explicit %s across all environments", (mcpPolicy) => {
      for (const environmentType of ["dev", "test", "prod"] as WorkspaceEnvironmentType[]) {
        expect(resolveWorkspaceMcpPolicy({ environmentType, mcpPolicy })).toBe(mcpPolicy);
      }
    },
  );
});
