// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SshTaskDetail, SshTaskRun, WorkspaceEnvironment, WorkspaceVariable } from "@unfour/command-client";

vi.mock("@unfour/command-client", () => ({
  getSshTask: vi.fn(), listSshTaskRuns: vi.fn(), listWorkspaceEnvironments: vi.fn(),
  listWorkspaceVariables: vi.fn(), runSshTask: vi.fn(),
}));
import { getSshTask, listSshTaskRuns, listWorkspaceEnvironments, listWorkspaceVariables, runSshTask } from "@unfour/command-client";
import { useTaskRunDialogState } from "./useTaskRunDialogState";

const variable: WorkspaceVariable = {
  id: "workspace-var", workspaceId: "w", key: "REGION", value: "workspace-private", isSecret: true, isEnabled: true,
  description: null, sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null, revision: 3,
};
const environment: WorkspaceEnvironment = {
  id: "env", workspaceId: "w", name: "Environment", isActive: true, sortOrder: 0,
  createdAt: "", updatedAt: "", deletedAt: null, revision: 1,
  variables: [{ ...variable, id: "environment-var", environmentId: "env", value: "environment-public", isSecret: false }],
};
const detail: SshTaskDetail = {
  task: { id: "task", workspaceId: "w", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null },
  steps: [{ id: "step", workspaceId: "w", taskId: "task", name: "Command", stepType: "command", position: 0, enabled: true,
    configVersion: 1, configJson: { command: "echo {{REGION}} {{PUBLIC}}" }, createdAt: "", updatedAt: "", deletedAt: null }],
  localBinding: null, detectedInputs: [],
};
const previous: SshTaskRun = {
  id: "past", workspaceId: "w", taskId: "task", connectionId: "conn", status: "success", startedAt: "", finishedAt: "", logPath: "", errorMessage: null,
  context: { version: 1, environmentId: "env", environmentUpdatedAt: "", taskVersion: "", inputs: {
    REGION: { source: "workspace" }, PUBLIC: { source: "manual", value: "ordinary" },
  } },
};
const clients: QueryClient[] = [];

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getSshTask).mockResolvedValue(detail);
  vi.mocked(listWorkspaceVariables).mockResolvedValue([variable]);
  vi.mocked(listWorkspaceEnvironments).mockResolvedValue([environment]);
  vi.mocked(listSshTaskRuns).mockResolvedValue([previous]);
  vi.mocked(runSshTask).mockResolvedValue(previous);
});
afterEach(() => { cleanup(); clients.splice(0).forEach((client) => client.clear()); });

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity }, mutations: { retry: false, gcTime: Infinity } } });
  clients.push(client);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(() => useTaskRunDialogState({ workspaceId: "w", handleError: vi.fn(), onStarted: vi.fn() }), { wrapper });
}

describe("task input forced sensitivity", () => {
  it.each(["recall", "rerun"])("locks a historical Workspace Secret shadowed by a public environment on %s", async (entry) => {
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task", entry === "rerun" ? previous : undefined); });
    if (entry === "recall") await act(async () => { await result.current.restorePreviousRun(); });
    expect(result.current.runInputs.REGION).toBe("workspace-private");
    expect(result.current.runAutomaticSecretInputs).toEqual(["REGION"]);
    act(() => result.current.toggleSecretInput("REGION"));
    expect(result.current.runSecretInputs).toContain("REGION");
    act(() => result.current.setRunInputs((inputs) => ({ ...inputs, REGION: "edited-private" })));
    act(() => result.current.toggleSecretInput("REGION"));
    expect(result.current.runAutomaticSecretInputs).toEqual(["REGION"]);
    expect(result.current.runSecretInputs).toContain("REGION");
    await act(async () => { await result.current.runMutation.mutateAsync(); });
    expect(runSshTask).toHaveBeenCalledWith(expect.objectContaining({
      inputs: { REGION: "edited-private", PUBLIC: "ordinary" }, secretInputNames: ["REGION"],
      context: expect.objectContaining({ inputSources: { REGION: "manual", PUBLIC: "manual" }, inputVersions: {} }),
    }));
  });

  it("submits the actual historical source and keeps variable values out of the request", async () => {
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task", previous); });
    act(() => result.current.toggleSecretInput("REGION"));
    await act(async () => { await result.current.runMutation.mutateAsync(); });
    expect(runSshTask).toHaveBeenCalledWith(expect.objectContaining({
      inputs: { REGION: "", PUBLIC: "ordinary" }, secretInputNames: ["REGION"],
      context: expect.objectContaining({ inputSources: { REGION: "workspace", PUBLIC: "manual" }, inputVersions: { REGION: { id: "workspace-var", revision: 3 } } }),
    }));
  });

  it("resets provenance with environment defaults and leaves ordinary fields freely toggleable", async () => {
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    expect(result.current.runAutomaticSecretInputs).toEqual([]);
    act(() => result.current.toggleSecretInput("PUBLIC"));
    expect(result.current.runSecretInputs).toContain("PUBLIC");
    act(() => result.current.toggleSecretInput("PUBLIC"));
    expect(result.current.runSecretInputs).not.toContain("PUBLIC");
    await act(async () => { await result.current.prepareRun("task", previous); });
    const scope = result.current.runInputScopeKey;
    expect(result.current.runAutomaticSecretInputs).toEqual(["REGION"]);
    await act(async () => { await result.current.changeRunEnvironment("env"); });
    expect(result.current.runInputs.REGION).toBe("environment-public");
    expect(result.current.runAutomaticSecretInputs).toEqual([]);
    expect(result.current.runSecretInputs).toEqual([]);
    expect(result.current.runInputScopeKey).not.toBe(scope);
    act(() => result.current.toggleSecretInput("REGION"));
    expect(result.current.runSecretInputs).toContain("REGION");
    act(() => result.current.toggleSecretInput("REGION"));
    expect(result.current.runSecretInputs).not.toContain("REGION");
    await act(async () => { await result.current.changeRunEnvironment(""); });
    expect(result.current.runInputs.REGION).toBe("workspace-private");
    expect(result.current.runAutomaticSecretInputs).toEqual(["REGION"]);
  });

  it.each(["removed", "public", "disabled", "deleted"])("keeps historical sensitivity locked when the selected secret source becomes %s", async (change) => {
    const saved = { ...previous, context: { ...previous.context!, inputs: { REGION: { source: "workspace" as const, secret: true } } } };
    vi.mocked(listWorkspaceVariables).mockResolvedValue(change === "removed" ? [] : [{ ...variable,
      isSecret: change !== "public", isEnabled: change !== "disabled", deletedAt: change === "deleted" ? "now" : null }]);
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task", saved); });
    expect(result.current.runInputs.REGION).toBe(change === "public" ? "workspace-private" : "");
    expect(result.current.runAutomaticSecretInputs).toContain("REGION");
    act(() => result.current.toggleSecretInput("REGION"));
    expect(result.current.runSecretInputs).toContain("REGION");
    await act(async () => { await result.current.changeRunEnvironment("env"); });
    expect(result.current.runAutomaticSecretInputs).toEqual([]);
    expect(result.current.runSecretInputs).toEqual([]);
    expect(result.current.runInputs.REGION).toBe("environment-public");
  });
});
