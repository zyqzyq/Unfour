import { expect, it, vi } from "vitest";
import { call } from "./invoke";
import { isTemporarySshTaskRun, runTemporarySshTask } from "./ssh";
import type { SshTaskRun, SshTaskRunInput } from "../types";

vi.mock("./invoke", () => ({ call: vi.fn(), isTauriRuntime: () => true }));

it("passes the temporary handle only as an adapter argument and never as a saved connection", async () => {
  const input: SshTaskRunInput = {
    workspaceId: "workspace", taskId: "task", connectionId: "saved-default",
    inputs: { VERSION: "v1" }, secretInputNames: [],
    context: { environmentId: null, environmentUpdatedAt: null, taskVersion: "revision", inputSources: { VERSION: "manual" }, inputVersions: {} },
  };
  await runTemporarySshTask(input, "temporary-handle");
  expect(call).toHaveBeenCalledExactlyOnceWith("ssh_temporary_task_run", { input: { ...input, connectionId: null }, handle: "temporary-handle" });
  expect(input.connectionId).toBe("saved-default");
  expect(JSON.stringify(vi.mocked(call).mock.calls[0][1]?.input)).not.toContain("temporary-handle");
});

it("identifies temporary results using the explicit runtime marker", () => {
  expect(isTemporarySshTaskRun({ warnings: ["SSH_TASK_TEMPORARY_RUN"] } as SshTaskRun)).toBe(true);
  expect(isTemporarySshTaskRun({ connectionId: null } as SshTaskRun)).toBe(false);
});

it("passes terminal execution only as an explicit one-run adapter option", async () => {
  vi.mocked(call).mockClear();
  const input: SshTaskRunInput = { workspaceId: "w", taskId: "task", connectionId: null, inputs: {}, secretInputNames: [] };
  await runTemporarySshTask(input, "live", "terminal");
  expect(call).toHaveBeenCalledExactlyOnceWith("ssh_temporary_task_run", { input, handle: "live", executionMode: "terminal" });
  expect(input).not.toHaveProperty("executionMode");
});
