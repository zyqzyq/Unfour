// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SshTaskDetail, SshTaskRun, SshTaskRunEvent } from "@unfour/command-client";

const warningMock = vi.hoisted(() => vi.fn());
vi.mock("@unfour/ui", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/ui")>(),
  useFeedback: () => ({ info: warningMock }),
}));

vi.mock("@unfour/command-client", () => ({
  cancelSshTaskRun: vi.fn(), clearSshTaskRuns: vi.fn(), getSshTask: vi.fn(),
  listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(),
  readSshTaskRunLog: vi.fn(), registerSshTaskRunChannel: vi.fn(), runSshTask: vi.fn(),
}));

import {
  cancelSshTaskRun, clearSshTaskRuns, getSshTask, listWorkspaceEnvironments,
  listWorkspaceVariables, readSshTaskRunLog, registerSshTaskRunChannel, runSshTask,
} from "@unfour/command-client";
import { useSshTaskRunSession } from "./useSshTaskRunSession";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

const detail: SshTaskDetail = {
  task: { id: "task", workspaceId: "w1", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null },
  steps: [], localBinding: null, detectedInputs: [],
};

function run(id = "run", taskId = "task"): SshTaskRun {
  return { id, taskId, workspaceId: "w1", connectionId: "connection", status: "running", startedAt: "", finishedAt: null, errorMessage: null, logPath: "" };
}

function event(partial: Partial<SshTaskRunEvent> = {}): SshTaskRunEvent {
  return {
    runId: "run", taskId: "task", kind: "output", stepId: null, stepName: null,
    stepType: null, position: null, status: null, stream: "stdout", data: "output",
    exitCode: null, durationMs: null, direction: null, transferredBytes: null,
    totalBytes: null, bytesPerSecond: null, error: null, createdAt: "2026-01-01T00:00:00Z",
    ...partial,
  };
}

const subscriptions: {
  listener: (event: SshTaskRunEvent) => void;
  registration: ReturnType<typeof deferred<() => void>>;
  dispose: ReturnType<typeof vi.fn>;
}[] = [];
const clients: QueryClient[] = [];

beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  subscriptions.length = 0;
  vi.mocked(registerSshTaskRunChannel).mockImplementation((listener) => {
    const entry = { listener, registration: deferred<() => void>(), dispose: vi.fn() };
    subscriptions.push(entry);
    return entry.registration.promise;
  });
  vi.mocked(listWorkspaceEnvironments).mockResolvedValue([]);
  vi.mocked(listWorkspaceVariables).mockResolvedValue([]);
  vi.mocked(getSshTask).mockResolvedValue(detail);
  vi.mocked(readSshTaskRunLog).mockResolvedValue("history log");
  vi.mocked(runSshTask).mockResolvedValue(run());
  vi.mocked(cancelSshTaskRun).mockResolvedValue(run());
});

afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function setup(active = true, strict = false) {
  const client = new QueryClient({ defaultOptions: {
    queries: { retry: false, gcTime: Infinity }, mutations: { retry: false, gcTime: Infinity },
  } });
  clients.push(client);
  const handleError = vi.fn();
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook((props) => useSshTaskRunSession({ ...props, handleError }), {
    wrapper, reactStrictMode: strict, initialProps: { active, workspaceId: "w1" },
  });
  await act(async () => {});
  return { ...hook, client, handleError, invalidate };
}

async function tick(ms: number) {
  await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
}

describe("task run subscription lifecycle", () => {
  it("batches received events at 16 ms and invalidates only completed runs", async () => {
    const { result, invalidate } = await setup();
    const events = [event(), event({ kind: "step", status: "failed" }),
      event({ kind: "run", status: "running" }), event({ kind: "run", status: "success" })];
    act(() => events.forEach(subscriptions[0].listener));
    await tick(15);
    expect(result.current.eventsByRun).toEqual({});
    expect(invalidate).not.toHaveBeenCalled();
    await tick(1);
    expect(result.current.eventsByRun.run).toEqual(events);
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
  });

  it("buffers while hidden and resumes at the next 250 ms poll without resubscribing", async () => {
    const { result, rerender, invalidate } = await setup(false);
    act(() => subscriptions[0].listener(event({ kind: "run", status: "failed" })));
    await tick(516);
    expect(result.current.eventsByRun).toEqual({});
    expect(invalidate).not.toHaveBeenCalled();
    rerender({ active: true, workspaceId: "w1" });
    await tick(249);
    expect(result.current.eventsByRun).toEqual({});
    await tick(1);
    expect(result.current.eventsByRun.run).toHaveLength(1);
    expect(registerSshTaskRunChannel).toHaveBeenCalledTimes(1);
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
  });

  it("discards pending and late events on unmount, then disposes a late registration", async () => {
    const { unmount, invalidate } = await setup();
    const subscription = subscriptions[0];
    act(() => subscription.listener(event({ kind: "run", status: "success" })));
    unmount();
    subscription.listener(event({ kind: "run", status: "success" }));
    await act(async () => subscription.registration.resolve(subscription.dispose));
    await tick(1000);
    expect(subscription.dispose).toHaveBeenCalledTimes(1);
    expect(invalidate).not.toHaveBeenCalled();
    expect(cancelSshTaskRun).not.toHaveBeenCalled();
  });

  it("disposes an established subscription and cancels its hidden polling timer", async () => {
    const { unmount, invalidate } = await setup(false);
    const subscription = subscriptions[0];
    await act(async () => subscription.registration.resolve(subscription.dispose));
    act(() => subscription.listener(event({ kind: "run", status: "success" })));
    await tick(16);
    unmount();
    await tick(1000);
    expect(subscription.dispose).toHaveBeenCalledTimes(1);
    expect(invalidate).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("replaces the subscription on workspace change and discards the previous pending batch", async () => {
    const { result, rerender, invalidate } = await setup();
    const previous = subscriptions[0];
    act(() => previous.listener(event({ runId: "old", kind: "run", status: "success" })));
    rerender({ active: true, workspaceId: "w2" });
    await act(async () => previous.registration.resolve(previous.dispose));
    act(() => {
      previous.listener(event({ runId: "late" }));
      subscriptions[1].listener(event({ runId: "new", kind: "run", status: "cancelled" }));
    });
    await tick(16);
    expect(Object.keys(result.current.eventsByRun)).toEqual(["new"]);
    expect(previous.dispose).toHaveBeenCalledTimes(1);
    expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ssh-task-runs", "w2", "task"] });
    expect(registerSshTaskRunChannel).toHaveBeenCalledTimes(2);
  });

  it("keeps only the current StrictMode subscription alive", async () => {
    const { result, unmount } = await setup(true, true);
    expect(subscriptions).toHaveLength(2);
    await act(async () => subscriptions.forEach((entry) => entry.registration.resolve(entry.dispose)));
    act(() => subscriptions.forEach((entry, index) => entry.listener(event({ data: `${index}` }))));
    await tick(16);
    expect(result.current.eventsByRun.run.map((item) => item.data)).toEqual(["1"]);
    expect(subscriptions[0].dispose).toHaveBeenCalledTimes(1);
    expect(subscriptions[1].dispose).not.toHaveBeenCalled();
    unmount();
    expect(subscriptions[1].dispose).toHaveBeenCalledTimes(1);
  });
});

describe("task run output bounds", () => {
  it("caps hidden pending output at 10000 before applying the per-run cache bound", async () => {
    const { result, rerender, invalidate } = await setup(false);
    act(() => {
      subscriptions[0].listener(event({ kind: "run", status: "success" }));
      for (let index = 0; index < 10000; index++) subscriptions[0].listener(event({ data: `${index}` }));
    });
    await tick(16);
    rerender({ active: true, workspaceId: "w1" });
    await tick(250);
    expect(result.current.eventsByRun.run).toHaveLength(5000);
    expect(result.current.eventsByRun.run[0].data).toBe("5000");
    expect(result.current.eventsByRun.run.at(-1)?.data).toBe("9999");
    expect(invalidate).not.toHaveBeenCalled();
  });

  it("uses the latest selected run when pruning and compacts transfer progress", async () => {
    const { result } = await setup();
    act(() => {
      for (let index = 0; index < 16; index++) subscriptions[0].listener(event({ runId: `run-${index}` }));
    });
    await tick(16);
    act(() => result.current.setActiveRun(run("run-0")));
    act(() => {
      subscriptions[0].listener(event({ runId: "new", kind: "transfer", stepId: "upload", transferredBytes: 1 }));
      subscriptions[0].listener(event({ runId: "new", kind: "transfer", stepId: "upload", transferredBytes: 2 }));
    });
    await tick(16);
    expect(Object.keys(result.current.eventsByRun)).toHaveLength(16);
    expect(result.current.eventsByRun["run-0"]).toBeDefined();
    expect(result.current.eventsByRun["run-1"]).toBeUndefined();
    expect(result.current.eventsByRun.new).toEqual([event({ runId: "new", kind: "transfer", stepId: "upload", transferredBytes: 2 })]);
    expect(registerSshTaskRunChannel).toHaveBeenCalledTimes(1);
  });
});

describe("task run history cache", () => {
  it("selects task detail before loading a log, then reuses even an empty cached log", async () => {
    const { result } = await setup();
    const log = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValue(log.promise);
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run()); });
    expect(result.current.activeRun?.id).toBe("run");
    expect(result.current.activeRunTask).toEqual(detail);
    expect(result.current.historyLogLoading).toBe(true);
    await act(async () => { log.resolve(""); await opening; });
    expect(result.current.historyLogLoading).toBe(false);
    expect(result.current.historyLogByRun).toEqual({ run: "" });
    await act(async () => { await result.current.openHistoryRun(run()); });
    expect(readSshTaskRunLog).toHaveBeenCalledExactlyOnceWith("w1", "run");
  });

  it("uses committed live events instead of fetching a historical log", async () => {
    const { result } = await setup();
    act(() => subscriptions[0].listener(event()));
    await tick(16);
    await act(async () => { await result.current.openHistoryRun(run()); });
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
    expect(result.current.activeRun?.id).toBe("run");
    expect(result.current.historyLogLoading).toBe(false);
  });

  it("reports log failure and caches an empty result without retrying on reopen", async () => {
    const { result, handleError } = await setup();
    const failure = new Error("unavailable");
    vi.mocked(readSshTaskRunLog).mockRejectedValue(failure);
    await act(async () => { await result.current.openHistoryRun(run()); });
    expect(handleError).toHaveBeenCalledExactlyOnceWith(failure, { key: "feedback.ssh.taskLogLoadFailed" });
    expect(result.current.historyLogByRun).toEqual({ run: "" });
    expect(result.current.historyLogLoading).toBe(false);
    await act(async () => { await result.current.openHistoryRun(run()); });
    expect(readSshTaskRunLog).toHaveBeenCalledTimes(1);
  });

  it("reports task detail failure before changing selection or reading a log", async () => {
    const { result, handleError } = await setup();
    const failure = new Error("missing task");
    vi.mocked(getSshTask).mockRejectedValue(failure);
    await act(async () => { await result.current.openHistoryRun(run()); });
    expect(handleError).toHaveBeenCalledExactlyOnceWith(failure, { key: "feedback.ssh.taskLoadFailed" });
    expect(result.current.activeRun).toBeNull();
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
  });

  it("keeps the first completed log when reads overlap and bounds history to four runs", async () => {
    const { result } = await setup();
    const first = deferred<string>();
    const second = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    let opening1!: Promise<void>;
    let opening2!: Promise<void>;
    await act(async () => { opening1 = result.current.openHistoryRun(run()); });
    await act(async () => { opening2 = result.current.openHistoryRun(run()); });
    await act(async () => { second.resolve("second response"); await opening2; });
    await act(async () => { first.resolve("first response"); await opening1; });
    expect(result.current.historyLogByRun.run).toBe("second response");
    for (let index = 0; index < 4; index++) {
      await act(async () => { await result.current.openHistoryRun(run(`history-${index}`)); });
    }
    expect(Object.keys(result.current.historyLogByRun)).toEqual(["history-0", "history-1", "history-2", "history-3"]);
  });
});

describe("task mutations and output ownership", () => {
  it("prepares and starts a task, retains its detail, then cancels with the selected run id", async () => {
    const { result, invalidate } = await setup();
    await act(async () => { await result.current.prepareRun("task"); });
    expect(result.current.runDialogTask).toEqual(detail);
    act(() => result.current.setRunInputs({ ARG: "fixture" }));
    await act(async () => { await result.current.runMutation.mutateAsync(); });
    expect(runSshTask).toHaveBeenCalledExactlyOnceWith({
      workspaceId: "w1", taskId: "task", connectionId: null, inputs: { ARG: "fixture" }, secretInputNames: [],
    });
    expect(result.current.activeRunTask).toEqual(detail);
    expect(result.current.runDialogTask).toBeNull();
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
    await act(async () => { await result.current.cancelMutation.mutateAsync(); });
    expect(cancelSshTaskRun).toHaveBeenCalledExactlyOnceWith({ workspaceId: "w1", runId: "run" });
  });

  it("clears only the selected task's live cache and history logs after successful cleanup", async () => {
    const { result, invalidate } = await setup();
    await act(async () => { await result.current.openHistoryRun(run("other-history", "other-task")); });
    await act(async () => { await result.current.openHistoryRun(run("history")); });
    act(() => {
      subscriptions[0].listener(event());
      subscriptions[0].listener(event({ runId: "other", taskId: "other-task" }));
    });
    await tick(16);
    await act(async () => { await result.current.clearMutation.mutateAsync("task"); });
    expect(clearSshTaskRuns).toHaveBeenCalledExactlyOnceWith({ workspaceId: "w1", taskId: "task" });
    expect(Object.keys(result.current.eventsByRun)).toEqual(["other"]);
    expect(result.current.historyLogByRun).toEqual({ "other-history": "history log" });
    expect(result.current.activeRun?.id).toBe("history");
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
    await act(async () => { await result.current.openHistoryRun(run("history")); });
    expect(readSshTaskRunLog).toHaveBeenCalledTimes(3);
  });

  it("finishes another task's in-flight history log after cleanup succeeds", async () => {
    const { result, handleError } = await setup();
    const otherLog = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValue(otherLog.promise);
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run("other-run", "other-task")); });
    expect(result.current.historyLogLoading).toBe(true);
    await act(async () => { await result.current.clearMutation.mutateAsync("task"); });
    expect(result.current.historyLogLoading).toBe(true);
    expect(result.current.activeRun?.id).toBe("other-run");
    await act(async () => { otherLog.resolve("other log"); await opening; });
    expect(result.current.historyLogByRun).toEqual({ "other-run": "other log" });
    expect(result.current.historyLogLoading).toBe(false);
    expect(handleError).not.toHaveBeenCalled();
  });

  it("retains output when cleanup fails and reports cancel/cleanup failures", async () => {
    const { result, handleError } = await setup();
    await act(async () => { await result.current.openHistoryRun(run("other-history", "other-task")); });
    await act(async () => { await result.current.openHistoryRun(run()); });
    act(() => {
      subscriptions[0].listener(event());
      subscriptions[0].listener(event({ runId: "other", taskId: "other-task" }));
    });
    await tick(16);
    const failure = new Error("denied");
    vi.mocked(clearSshTaskRuns).mockRejectedValue(failure);
    vi.mocked(cancelSshTaskRun).mockRejectedValue(failure);
    await act(async () => { await expect(result.current.clearMutation.mutateAsync("task")).rejects.toBe(failure); });
    await act(async () => { await expect(result.current.cancelMutation.mutateAsync()).rejects.toBe(failure); });
    expect(result.current.eventsByRun.run).toHaveLength(1);
    expect(result.current.eventsByRun.other).toHaveLength(1);
    expect(result.current.historyLogByRun).toEqual({
      "other-history": "history log",
      run: "history log",
    });
    expect(handleError).toHaveBeenCalledWith(failure, { key: "feedback.ssh.taskHistoryClearFailed" });
    expect(handleError).toHaveBeenCalledWith(failure, { key: "feedback.ssh.taskCancelFailed" });
  });
});

describe("task history selection and cleanup races", () => {
  it.each([
    { success: true, active: true }, { success: true, active: false },
    { success: false, active: true }, { success: false, active: false },
  ])("handles output during cleanup with success=$success active=$active", async ({ success, active }) => {
    const { result, rerender, handleError } = await setup(active);
    const clearing = deferred<Awaited<ReturnType<typeof clearSshTaskRuns>>>();
    const log = deferred<string>();
    vi.mocked(clearSshTaskRuns).mockReturnValue(clearing.promise);
    vi.mocked(readSshTaskRunLog).mockReturnValue(log.promise);
    let opening!: Promise<void>;
    let cleanupResult!: Promise<unknown>;
    await act(async () => { opening = result.current.openHistoryRun(run()); });
    await act(async () => { cleanupResult = result.current.clearMutation.mutateAsync("task").catch((error) => error); });
    act(() => subscriptions[0].listener(event({ data: "during cleanup" })));
    await tick(16);
    expect(result.current.eventsByRun).toEqual(active ? { run: [event({ data: "during cleanup" })] } : {});
    const failure = new Error("cleanup denied");
    await act(async () => {
      if (success) clearing.resolve({ deletedRuns: 1, deletedLogs: 1 });
      else clearing.reject(failure);
      await cleanupResult;
    });
    act(() => subscriptions[0].listener(event({ data: "after cleanup" })));
    await act(async () => { log.resolve("in-flight log"); await opening; });
    rerender({ active: true, workspaceId: "w1" });
    await tick(active ? 16 : 250);
    expect(result.current.eventsByRun.run.map((item) => item.data)).toEqual(
      success ? ["after cleanup"] : ["during cleanup", "after cleanup"],
    );
    expect(result.current.historyLogByRun).toEqual(success ? {} : { run: "in-flight log" });
    if (success) expect(handleError).not.toHaveBeenCalled();
    else expect(handleError).toHaveBeenCalledExactlyOnceWith(failure, { key: "feedback.ssh.taskHistoryClearFailed" });
  });

  it("keeps the latest selection when task details resolve in reverse order", async () => {
    const { result } = await setup();
    const oldDetail = deferred<SshTaskDetail>();
    vi.mocked(getSshTask).mockReturnValueOnce(oldDetail.promise).mockResolvedValueOnce({
      ...detail, task: { ...detail.task, id: "new-task" },
    });
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run("old")); });
    await act(async () => { await result.current.openHistoryRun(run("new", "new-task")); });
    await act(async () => { oldDetail.resolve(detail); await opening; });
    expect(result.current.activeRun?.id).toBe("new");
    expect(result.current.activeRunTask?.task.id).toBe("new-task");
    expect(readSshTaskRunLog).toHaveBeenCalledExactlyOnceWith("w1", "new");
  });

  it("allows another task's pending selection to finish after cleanup", async () => {
    const { result } = await setup();
    const taskDetail = deferred<SshTaskDetail>();
    vi.mocked(getSshTask).mockReturnValue(taskDetail.promise);
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run("other-run", "other-task")); });
    await act(async () => { await result.current.clearMutation.mutateAsync("task"); });
    const otherDetail = { ...detail, task: { ...detail.task, id: "other-task" } };
    await act(async () => { taskDetail.resolve(otherDetail); await opening; });
    expect(result.current.activeRun?.id).toBe("other-run");
    expect(result.current.activeRunTask).toEqual(otherDetail);
    expect(readSshTaskRunLog).toHaveBeenCalledExactlyOnceWith("w1", "other-run");
  });

  it.each([
    { action: "clear", reject: false }, { action: "clear", reject: true },
    { action: "close", reject: false }, { action: "close", reject: true },
    { action: "workspace", reject: false }, { action: "workspace", reject: true },
    { action: "unmount", reject: false }, { action: "unmount", reject: true },
  ])("invalidates pending task details on $action with reject=$reject", async ({ action, reject }) => {
    const { result, rerender, unmount, handleError } = await setup();
    const taskDetail = deferred<SshTaskDetail>();
    vi.mocked(getSshTask).mockReturnValue(taskDetail.promise);
    let opening!: Promise<void>;
    await act(async () => { opening = result.current.openHistoryRun(run()); });
    if (action === "clear") await act(async () => { await result.current.clearMutation.mutateAsync("task"); });
    if (action === "close") act(() => result.current.setActiveRun(null));
    if (action === "workspace") rerender({ active: true, workspaceId: "w2" });
    if (action === "unmount") unmount();
    await act(async () => {
      if (reject) taskDetail.reject(new Error("stale task detail"));
      else taskDetail.resolve(detail);
      await opening;
    });
    expect(result.current.activeRun).toBeNull();
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
    expect(handleError).not.toHaveBeenCalled();
  });

  it.each([true, false])("ignores old workspace cleanup success=%s after a new run is selected", async (success) => {
    const { result, rerender, handleError, invalidate } = await setup();
    const clearing = deferred<Awaited<ReturnType<typeof clearSshTaskRuns>>>();
    vi.mocked(clearSshTaskRuns).mockReturnValue(clearing.promise);
    let cleanupResult!: Promise<unknown>;
    await act(async () => { cleanupResult = result.current.clearMutation.mutateAsync("task").catch((error) => error); });
    rerender({ active: true, workspaceId: "w2" });
    await act(async () => { await result.current.openHistoryRun({ ...run(), workspaceId: "w2" }); });
    act(() => subscriptions[1].listener(event({ data: "new workspace" })));
    await tick(16);
    await act(async () => {
      if (success) clearing.resolve({ deletedRuns: 1, deletedLogs: 1 });
      else clearing.reject(new Error("old workspace cleanup"));
      await cleanupResult;
    });
    expect(result.current.eventsByRun.run.map((item) => item.data)).toEqual(["new workspace"]);
    expect(result.current.historyLogByRun).toEqual({ run: "history log" });
    expect(result.current.activeRun?.workspaceId).toBe("w2");
    expect(handleError).not.toHaveBeenCalled();
    if (success) expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
    else expect(invalidate).not.toHaveBeenCalled();
  });

  it.each([true, false])("avoids output updates and feedback after unmounted cleanup with success=%s", async (success) => {
    const { result, unmount, handleError, invalidate } = await setup();
    const clearing = deferred<Awaited<ReturnType<typeof clearSshTaskRuns>>>();
    vi.mocked(clearSshTaskRuns).mockReturnValue(clearing.promise);
    let cleanupResult!: Promise<unknown>;
    await act(async () => { cleanupResult = result.current.clearMutation.mutateAsync("task").catch((error) => error); });
    const snapshot = result.current;
    unmount();
    await act(async () => {
      if (success) clearing.resolve({ deletedRuns: 1, deletedLogs: 1 });
      else clearing.reject(new Error("unmounted cleanup"));
      await cleanupResult;
    });
    expect(result.current).toBe(snapshot);
    expect(handleError).not.toHaveBeenCalled();
    if (success) expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["ssh-task-runs", "w1", "task"] });
    else expect(invalidate).not.toHaveBeenCalled();
  });
});

it("keeps the task handle and warns after activity logging fails", async () => {
  const output = { ...run(), warnings: ["ACTIVITY_RECORD_FAILED"] };
  vi.mocked(runSshTask).mockResolvedValue(output);
  vi.mocked(cancelSshTaskRun).mockResolvedValue(output);
  const { result, handleError } = await setup();
  await act(async () => { await result.current.prepareRun("task"); });
  await act(async () => { await result.current.runMutation.mutateAsync(); });
  expect(result.current.activeRun?.id).toBe("run");
  expect(result.current.runDialogTask).toBeNull();
  expect(warningMock).toHaveBeenCalledOnce();
  expect(runSshTask).toHaveBeenCalledOnce();
  await act(async () => { await result.current.cancelMutation.mutateAsync(); });
  expect(cancelSshTaskRun).toHaveBeenCalledWith({ workspaceId: "w1", runId: "run" });
  expect(warningMock).toHaveBeenCalledTimes(2);
  expect(handleError).not.toHaveBeenCalled();
});

it("keeps cleared task history when activity logging fails", async () => {
  vi.mocked(clearSshTaskRuns).mockResolvedValue({
    deletedRuns: 2,
    deletedLogs: 1,
    warnings: ["ACTIVITY_RECORD_FAILED"],
  });
  const { result, handleError } = await setup();
  await act(async () => { await result.current.clearMutation.mutateAsync("task"); });
  expect(clearSshTaskRuns).toHaveBeenCalledExactlyOnceWith({ workspaceId: "w1", taskId: "task" });
  expect(warningMock).toHaveBeenCalledOnce();
  expect(handleError).not.toHaveBeenCalled();
});
