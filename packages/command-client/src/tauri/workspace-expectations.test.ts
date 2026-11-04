import { expect, it, vi } from "vitest";
import { replaceWorkspaceVariables, updateWorkspaceEnvironmentVariables } from "./workspace";
import { call } from "./invoke";
import type { WorkspaceEnvironment } from "../types";

vi.mock("./invoke", () => ({ call: vi.fn() }));

it("sends the desktop environment baseline as an optional Tauri argument", () => {
  const expected = { id: "e", revision: 3, variables: [] } as unknown as WorkspaceEnvironment;
  updateWorkspaceEnvironmentVariables("w", "e", "Dev", [], expected);
  expect(call).toHaveBeenLastCalledWith("workspace_environment_update", { workspaceId: "w", environmentId: "e", name: "Dev", variables: [], expected });
});

it("preserves an empty workspace baseline instead of omitting the concurrency check", () => {
  replaceWorkspaceVariables("w", [], []);
  expect(call).toHaveBeenLastCalledWith("workspace_variables_replace", { workspaceId: "w", variables: [], expected: [] });
});
