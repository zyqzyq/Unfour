// @vitest-environment jsdom
import { useState, type ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SshTaskDetail, SshTaskRun, SshTaskRunEvent } from "@unfour/command-client";

vi.mock("@unfour/ui", async (original) => ({
  ...await original<typeof import("@unfour/ui")>(),
  useFeedback: () => ({ info: vi.fn() }),
}));

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSshTaskRuns: vi.fn(), getSshTask: vi.fn(), readSshTaskRunLog: vi.fn(), registerSshTaskRunChannel: vi.fn(),
  listTemporaryRoutes: vi.fn(), listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(), runTemporarySshTask: vi.fn(),
}));

import { getSshTask, listSshTaskRuns, readSshTaskRunLog, registerSshTaskRunChannel, listTemporaryRoutes,
  listWorkspaceEnvironments, listWorkspaceVariables, runTemporarySshTask } from "@unfour/command-client";
import { useTemporaryTaskRunCache } from "./useTemporaryTaskRunCache";
import { useSshTaskRunSession } from "./useSshTaskRunSession";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => { resolve = accept; });
  return { promise, resolve };
}

function run(id: string, taskId = "task"): SshTaskRun {
  return {
    id, taskId, workspaceId: "workspace", connectionId: null,
    status: "running", startedAt: "", finishedAt: null,
    errorMessage: null, logPath: "", warnings: ["SSH_TASK_TEMPORARY_RUN"],
  };
}

const clients: QueryClient[] = [];
const detail: SshTaskDetail = {
  task: { id: "task", workspaceId: "workspace", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null },
  steps: [], localBinding: null, detectedInputs: [],
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  vi.mocked(getSshTask).mockResolvedValue(detail);
  vi.mocked(listSshTaskRuns).mockResolvedValue([]);
  vi.mocked(readSshTaskRunLog).mockResolvedValue("");
  vi.mocked(registerSshTaskRunChannel).mockResolvedValue(() => {});
  vi.mocked(listTemporaryRoutes).mockResolvedValue([{ handle: "live", displayName: "Node", endpoint: "host:22" }]);
  vi.mocked(listWorkspaceEnvironments).mockResolvedValue([]);
  vi.mocked(listWorkspaceVariables).mockResolvedValue([]);
});
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.useRealTimers();
});

function setup(initial: SshTaskRun | null) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  clients.push(client);
  const clearOutput = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook(() => {
    const [active, setActive] = useState<SshTaskRun | null>(initial);
    const { track, contains } = useTemporaryTaskRunCache("workspace", clearOutput, setActive);
    return { active, setActive, track, contains };
  }, { wrapper });
  return { ...hook, client, clearOutput };
}

function setupSession() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  clients.push(client);
  const handleError = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook(() => useSshTaskRunSession({ active: true, workspaceId: "workspace", handleError }), { wrapper });
  return { ...hook, handleError, client };
}

describe("useTemporaryTaskRunCache", () => {
  it("keeps a newly tracked run when an earlier poll completes without it", async () => {
    const first = run("first");
    const second = run("second");
    const pending = deferred<SshTaskRun[]>();
    vi.mocked(listSshTaskRuns).mockReturnValueOnce(pending.promise).mockResolvedValue([second]);
    const { result, client, clearOutput } = setup(first);
    const key = ["ssh-task-runs", "workspace", "task"];
    act(() => {
      result.current.track(first);
      client.setQueryData(key, [first]);
      vi.advanceTimersByTime(1_000);
    });
    expect(listSshTaskRuns).toHaveBeenCalledOnce();

    act(() => {
      result.current.track(second);
      result.current.setActive(second);
      client.setQueryData(key, [second, first]);
    });
    await act(async () => { pending.resolve([]); });

    expect(clearOutput.mock.calls).toEqual([[first.id]]);
    expect(result.current.active).toEqual(second);
    expect(client.getQueryData(key)).toEqual([second]);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(listSshTaskRuns).toHaveBeenCalledTimes(2);
    expect(clearOutput).not.toHaveBeenCalledWith(second.id);
    expect(result.current.active).toEqual(second);
  });

  it("does not clear a tracked ID that now belongs to another task", async () => {
    const first = run("run");
    const reassigned = run("run", "other-task");
    const pending = deferred<SshTaskRun[]>();
    vi.mocked(listSshTaskRuns).mockReturnValueOnce(pending.promise).mockResolvedValue([reassigned]);
    const { result, clearOutput } = setup(first);
    act(() => {
      result.current.track(first);
      vi.advanceTimersByTime(1_000);
      result.current.track(reassigned);
      result.current.setActive(reassigned);
    });
    await act(async () => { pending.resolve([]); });

    expect(clearOutput).not.toHaveBeenCalled();
    expect(result.current.active).toEqual(reassigned);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(listSshTaskRuns).toHaveBeenLastCalledWith("workspace", "other-task");
  });

  it("removes temporary history rows loaded without opening the run", async () => {
    const temporary = run("temporary");
    const saved: SshTaskRun = { ...run("saved"), connectionId: "connection", warnings: [] };
    vi.mocked(listSshTaskRuns).mockResolvedValue([saved]);
    const { result, client, clearOutput } = setup(null);
    const key = ["ssh-task-runs", "workspace", "task"];
    act(() => { client.setQueryData(key, [temporary, saved]); });
    expect(result.current.contains(temporary.id)).toBe(true);

    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(client.getQueryData(key)).toEqual([saved]);
    expect(clearOutput.mock.calls).toEqual([[temporary.id]]);
    expect(result.current.contains(temporary.id)).toBe(false);
    expect(result.current.active).toBeNull();
  });

  it("cancels a pending history query before discarding workspace rows", async () => {
    const pending = deferred<SshTaskRun[]>();
    const temporary = run("temporary");
    const saved: SshTaskRun = { ...run("saved"), connectionId: "connection", warnings: [] };
    const { client, unmount } = setup(null);
    const key = ["ssh-task-runs", "workspace", "task"];
    act(() => { client.setQueryData(key, [saved]); });
    const fetch = client.fetchQuery({ queryKey: key, queryFn: () => pending.promise }).catch(() => undefined);
    unmount();
    pending.resolve([temporary, saved]);
    await fetch;
    expect(client.getQueryData(key)).toEqual([saved]);
  });

  it("keeps a removed ID untracked after cancelling and reverting an older query", async () => {
    const pending = deferred<SshTaskRun[]>();
    const temporary = run("temporary");
    const { result, client } = setup(null);
    const key = ["ssh-task-runs", "workspace", "task"];
    act(() => { client.setQueryData(key, [temporary]); });
    const fetching = client.fetchQuery({ queryKey: key, queryFn: () => pending.promise }).catch(() => undefined);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.contains(temporary.id)).toBe(false);
    await act(async () => { pending.resolve([temporary]); await fetching; });
    expect(result.current.contains(temporary.id)).toBe(false);
    expect(client.getQueryData(key)).toEqual([]);
  });

  it("does not reopen a removed run when its pending task detail arrives", async () => {
    const pending = deferred<SshTaskDetail>();
    vi.mocked(getSshTask).mockReturnValue(pending.promise);
    const { result, handleError } = setupSession();
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run("removed")); });
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    await act(async () => { pending.resolve(detail); await opening; });
    expect(result.current.activeRun).toBeNull();
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
    expect(handleError).not.toHaveBeenCalled();
  });

  it("ignores an older history query before a removed run's pending detail arrives", async () => {
    const history = deferred<SshTaskRun[]>();
    const task = deferred<SshTaskDetail>();
    const temporary = run("removed");
    vi.mocked(getSshTask).mockReturnValue(task.promise);
    const { result, handleError, client } = setupSession();
    const key = ["ssh-task-runs", "workspace", "task"];
    act(() => { client.setQueryData(key, [temporary]); });
    const fetching = client.fetchQuery({ queryKey: key, queryFn: () => history.promise }).catch(() => undefined);
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(temporary); });
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(client.getQueryData(key)).toEqual([]);
    await act(async () => { history.resolve([temporary]); await fetching; });
    await act(async () => { task.resolve(detail); await opening; });
    expect(client.getQueryData(key)).toEqual([]);
    expect(result.current.activeRun).toBeNull();
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
    expect(handleError).not.toHaveBeenCalled();
  });

  it("purges output after an accepted run returns to an already closed dialog", async () => {
    const pending = deferred<SshTaskRun>();
    const temporary = run("accepted");
    let listener!: (event: SshTaskRunEvent) => void;
    vi.mocked(registerSshTaskRunChannel).mockImplementation(async (callback) => { listener = callback; return () => {}; });
    vi.mocked(runTemporarySshTask).mockReturnValue(pending.promise);
    const { result } = setupSession();
    await act(async () => { await result.current.prepareRun("task"); });
    act(() => { result.current.setRunTemporaryHandle("live"); result.current.setRunTemporaryExecutionMode("exec"); });
    let starting!: Promise<SshTaskRun>;
    await act(async () => { starting = result.current.runMutation.mutateAsync(); });
    expect(runTemporarySshTask).toHaveBeenCalledOnce();
    act(() => { result.current.closeRunDialog(); });
    await act(async () => { pending.resolve(temporary); await starting; });
    expect(result.current.activeRun).toBeNull();
    await act(async () => {
      listener({ runId: temporary.id, taskId: temporary.taskId, kind: "output", stepId: null, stepName: null, stepType: null,
        position: null, status: null, stream: "stdout", data: "temporary output", exitCode: null, durationMs: null,
        direction: null, transferredBytes: null, totalBytes: null, bytesPerSecond: null, error: null, createdAt: "" });
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(result.current.eventsByRun[temporary.id]).toHaveLength(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.eventsByRun[temporary.id]).toBeUndefined();
  });
});
