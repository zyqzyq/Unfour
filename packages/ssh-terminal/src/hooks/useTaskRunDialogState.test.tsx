// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SshTaskDetail, SshTaskRun, WorkspaceEnvironment, WorkspaceVariable } from "@unfour/command-client";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  getSshTask: vi.fn(), listSshTaskRuns: vi.fn(), listWorkspaceEnvironments: vi.fn(),
  listWorkspaceVariables: vi.fn(), runSshTask: vi.fn(), listTemporaryRoutes: vi.fn(), runTemporarySshTask: vi.fn(),
}));
import { getSshTask, listSshTaskRuns, listTemporaryRoutes, listWorkspaceEnvironments, listWorkspaceVariables, runSshTask, runTemporarySshTask } from "@unfour/command-client";
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
  vi.mocked(listTemporaryRoutes).mockResolvedValue([]);
});
afterEach(() => { cleanup(); clients.splice(0).forEach((client) => client.clear()); });

function setup(onAccepted = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity }, mutations: { retry: false, gcTime: Infinity } } });
  clients.push(client);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(({ workspaceId }) => useTaskRunDialogState({ workspaceId, handleError: vi.fn(), onStarted: vi.fn(), onAccepted }), { wrapper, initialProps: { workspaceId: "w" } });
}

describe("temporary Task targets", () => {
  const target = { handle: "live", displayName: "Node", endpoint: "10.0.0.2:22" };
  it("requires fresh terminal readiness for each selection and dispatches the explicit mode", async () => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([target]);
    vi.mocked(runTemporarySshTask).mockResolvedValue({ ...previous, context: null, connectionId: null, warnings: ["SSH_TASK_TEMPORARY_RUN"] });
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => result.current.setRunTemporaryHandle("live"));
    expect(result.current.runTemporaryExecutionMode).toBe("terminal");
    await act(async () => { await expect(result.current.runMutation.mutateAsync()).rejects.toThrow(/Confirm that the terminal/); });
    expect(runTemporarySshTask).not.toHaveBeenCalled();
    act(() => result.current.setRunTerminalReady(true));
    act(() => result.current.setRunTemporaryExecutionMode("exec"));
    expect(result.current.runTerminalReady).toBe(false);
    act(() => { result.current.setRunTemporaryExecutionMode("terminal"); result.current.setRunTerminalReady(true); });
    act(() => result.current.setRunTemporaryHandle("live"));
    expect(result.current.runTerminalReady).toBe(false);
    act(() => result.current.setRunTerminalReady(true));
    await act(async () => { await result.current.runMutation.mutateAsync(); });
    expect(runTemporarySshTask).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ connectionId: null }), "live", "terminal");
    await act(async () => { await result.current.prepareRun("task"); });
    expect(result.current.runTerminalReady).toBe(false);
    expect(result.current.runTemporaryHandle).toBeNull();
  });
  it.each(["busy", "recovery", "transfer"])("prevents terminal dispatch for %s even if the dialog handler is called directly", async (reason) => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([{ ...target, terminalTaskBusy: reason === "busy", terminalRecoveryRequired: reason === "recovery" }]);
    if (reason === "transfer") vi.mocked(getSshTask).mockResolvedValue({ ...detail, steps: [{ ...detail.steps[0], stepType: "upload" }] });
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTerminalReady(true); });
    await act(async () => { await expect(result.current.runMutation.mutateAsync()).rejects.toThrow(); });
    expect(runTemporarySshTask).not.toHaveBeenCalled();
    expect(runSshTask).not.toHaveBeenCalled();
  });
  it("does not restore an old terminal confirmation after an environment refresh", async () => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([target]);
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTerminalReady(true); });
    let finish!: (value: WorkspaceVariable[]) => void;
    vi.mocked(listWorkspaceVariables).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    let pending!: Promise<void>;
    await act(async () => { pending = result.current.changeRunEnvironment(""); });
    expect(result.current.runTerminalReady).toBe(false);
    // A stale enabled handler cannot mutate the target/confirmation mid-refresh.
    act(() => { result.current.setRunTemporaryHandle("other"); result.current.setRunTerminalReady(true); });
    expect(result.current.runTemporaryHandle).toBe("live");
    expect(result.current.runTerminalReady).toBe(false);
    await act(async () => { finish([variable]); await pending; });
    expect(result.current.runTerminalReady).toBe(false);
  });
  it("observes an accepted run even if its dialog closed while the adapter was pending", async () => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([target]);
    const ephemeral = { ...previous, context: null, connectionId: null, warnings: ["SSH_TASK_TEMPORARY_RUN"] };
    let finish!: (run: SshTaskRun) => void;
    vi.mocked(runTemporarySshTask).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const accepted = vi.fn();
    const { result } = setup(accepted);
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTemporaryExecutionMode("exec"); });
    let pending!: Promise<SshTaskRun>;
    await act(async () => { pending = result.current.runMutation.mutateAsync(); });
    expect(runTemporarySshTask).toHaveBeenCalledOnce();
    act(() => result.current.closeRunDialog());
    await act(async () => { finish(ephemeral); await pending; });
    expect(accepted).toHaveBeenCalledExactlyOnceWith(ephemeral);
    expect(result.current.runDialogTask).toBeNull();
  });
  it("uses the one-run adapter with no saved binding and preserves validation context", async () => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([target]);
    const ephemeral = { ...previous, context: null, connectionId: null, warnings: ["SSH_TASK_TEMPORARY_RUN"] };
    vi.mocked(runTemporarySshTask).mockResolvedValue(ephemeral);
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTemporaryExecutionMode("exec"); });
    await act(async () => { await result.current.runMutation.mutateAsync(); });
    expect(runTemporarySshTask).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ connectionId: null, workspaceId: "w", taskId: "task", context: expect.objectContaining({ taskVersion: expect.any(String) }) }), "live", "exec");
    expect(JSON.stringify(vi.mocked(runTemporarySshTask).mock.calls[0][0])).not.toContain("live");
    expect(runSshTask).not.toHaveBeenCalled();
    await act(async () => { await result.current.prepareRun("task", ephemeral); });
    expect(result.current.runTemporaryHandle).toBeNull();
    expect(result.current.runConnectionId).toBe("");
    expect(result.current.runNotice).toContain("do not save their target");
  });
  it("rechecks the live session before dispatch and never falls back to a saved connection", async () => {
    vi.mocked(listTemporaryRoutes).mockResolvedValue([target]);
    const { result } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTemporaryExecutionMode("exec"); });
    vi.mocked(listTemporaryRoutes).mockResolvedValue([]);
    await act(async () => { await expect(result.current.runMutation.mutateAsync()).rejects.toThrow(); });
    expect(runTemporarySshTask).not.toHaveBeenCalled();
    expect(runSshTask).not.toHaveBeenCalled();
  });
  it("cannot dispatch a pending live check after workspace changes", async () => {
    const { result, rerender } = setup();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTemporaryExecutionMode("exec"); });
    let finish!: (value: typeof target[]) => void;
    vi.mocked(listTemporaryRoutes).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    let pending!: Promise<unknown>;
    await act(async () => { pending = result.current.runMutation.mutateAsync().catch(() => undefined); });
    rerender({ workspaceId: "other" });
    await act(async () => { finish([target]); await pending; });
    expect(runTemporarySshTask).not.toHaveBeenCalled();
    expect(runSshTask).not.toHaveBeenCalled();
  });
});

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
