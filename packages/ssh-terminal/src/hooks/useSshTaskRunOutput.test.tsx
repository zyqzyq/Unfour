// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SshTaskRunEvent } from "@unfour/command-client";

vi.mock("@unfour/command-client", () => ({
  readSshTaskRunLog: vi.fn(), registerSshTaskRunChannel: vi.fn(),
}));

import { readSshTaskRunLog, registerSshTaskRunChannel } from "@unfour/command-client";
import { useSshTaskRunOutput } from "./useSshTaskRunOutput";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function event(partial: Partial<SshTaskRunEvent> = {}): SshTaskRunEvent {
  return {
    runId: "run", taskId: "task", kind: "output", stepId: null, stepName: null,
    stepType: null, position: null, status: null, stream: "stdout", data: "old output",
    exitCode: null, durationMs: null, direction: null, transferredBytes: null,
    totalBytes: null, bytesPerSecond: null, error: null, createdAt: "2026-01-01T00:00:00Z",
    ...partial,
  };
}

const listeners: ((event: SshTaskRunEvent) => void)[] = [];
const clients: QueryClient[] = [];

beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  listeners.length = 0;
  vi.mocked(registerSshTaskRunChannel).mockImplementation(async (listener) => {
    listeners.push(listener);
    return vi.fn();
  });
  vi.mocked(readSshTaskRunLog).mockResolvedValue("history log");
});

afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  vi.useRealTimers();
});

async function setup(active = true) {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: Infinity } } });
  clients.push(client);
  const handleError = vi.fn();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const hook = renderHook((props) => useSshTaskRunOutput({ ...props, handleError }), {
    wrapper, initialProps: { active, activeRunId: "run" as string | null, workspaceId: "w1" },
  });
  await act(async () => {});
  return { ...hook, handleError };
}

async function tick(ms: number) {
  await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
}

describe("task output cleanup races", () => {
  it.each(["resolve", "reject"] as const)("ignores a pre-clear log %s without ending a fresh read", async (outcome) => {
    const { result, handleError } = await setup();
    const oldLog = deferred<string>();
    const newLog = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValueOnce(oldLog.promise).mockReturnValueOnce(newLog.promise);
    let oldRead!: Promise<void>;
    let newRead!: Promise<void>;
    act(() => { oldRead = result.current.loadHistoryLog("run"); });
    expect(result.current.historyLogLoading).toBe(true);
    act(() => result.current.clearTaskOutput("task"));
    const loadingAfterClear = result.current.historyLogLoading;
    act(() => { newRead = result.current.loadHistoryLog("run"); });
    await act(async () => {
      if (outcome === "resolve") oldLog.resolve("deleted log");
      else oldLog.reject(new Error("deleted run"));
      await oldRead;
    });
    expect(result.current.historyLogByRun).toEqual({});
    expect(loadingAfterClear).toBe(false);
    expect(result.current.historyLogLoading).toBe(true);
    expect(handleError).not.toHaveBeenCalled();
    await act(async () => { newLog.resolve("fresh log"); await newRead; });
    expect(result.current.historyLogByRun).toEqual({ run: "fresh log" });
    expect(result.current.historyLogLoading).toBe(false);
  });

  it("invalidates all in-flight history logs along with the existing all-log cache reset", async () => {
    const { result } = await setup();
    const log = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValue(log.promise);
    let reading!: Promise<void>;
    act(() => { reading = result.current.loadHistoryLog("other-task-run"); });
    act(() => result.current.clearTaskOutput("task"));
    await act(async () => { log.resolve("old other log"); await reading; });
    expect(result.current.historyLogByRun).toEqual({});
  });

  it.each([true, false])("drops buffered old events and accepts fresh output with active=%s", async (active) => {
    const { result, rerender } = await setup(active);
    act(() => {
      listeners[0](event());
      listeners[0](event({ runId: "other", taskId: "other-task" }));
    });
    if (!active) await tick(16);
    act(() => {
      result.current.clearTaskOutput("task");
      listeners[0](event({ data: "fresh output" }));
    });
    if (!active) rerender({ active: true, activeRunId: "run", workspaceId: "w1" });
    await tick(active ? 16 : 250);
    expect(result.current.eventsByRun.run.map((item) => item.data)).toEqual(["fresh output"]);
    expect(result.current.eventsByRun.other).toEqual([event({ runId: "other", taskId: "other-task" })]);
    expect(registerSshTaskRunChannel).toHaveBeenCalledTimes(1);
    await act(async () => { await result.current.loadHistoryLog("run"); });
    expect(readSshTaskRunLog).not.toHaveBeenCalled();
  });

  it("updates cache ownership immediately when a flush and clear share a React batch", async () => {
    const { result } = await setup();
    act(() => listeners[0](event()));
    act(() => {
      vi.advanceTimersByTime(16);
      result.current.clearTaskOutput("task");
      listeners[0](event({ data: "new batch" }));
      vi.advanceTimersByTime(16);
    });
    expect(result.current.eventsByRun.run.map((item) => item.data)).toEqual(["new batch"]);
  });
});

describe("history read lifecycle and concurrency", () => {
  it.each(["resolve", "reject"] as const)("ignores an old workspace log %s and resets committed caches", async (outcome) => {
    const { result, rerender, handleError } = await setup();
    await act(async () => { await result.current.loadHistoryLog("cached"); });
    act(() => listeners[0](event({ runId: "cached-live" })));
    await tick(16);
    const oldLog = deferred<string>();
    const newLog = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValueOnce(oldLog.promise).mockReturnValueOnce(newLog.promise);
    let oldRead!: Promise<void>;
    let newRead!: Promise<void>;
    act(() => { oldRead = result.current.loadHistoryLog("run"); });
    rerender({ active: true, activeRunId: "run", workspaceId: "w2" });
    expect(result.current.historyLogByRun).toEqual({});
    expect(result.current.eventsByRun).toEqual({});
    expect(result.current.historyLogLoading).toBe(false);
    act(() => { newRead = result.current.loadHistoryLog("run"); });
    await act(async () => {
      if (outcome === "resolve") oldLog.resolve("old workspace");
      else oldLog.reject(new Error("old workspace"));
      await oldRead;
    });
    expect(result.current.historyLogByRun).toEqual({});
    expect(result.current.historyLogLoading).toBe(true);
    expect(handleError).not.toHaveBeenCalled();
    await act(async () => { newLog.resolve("new workspace"); await newRead; });
    expect(result.current.historyLogByRun).toEqual({ run: "new workspace" });
    expect(readSshTaskRunLog).toHaveBeenLastCalledWith("w2", "run");
  });

  it("keeps old reads invalid after switching away and back to the same workspace", async () => {
    const { result, rerender } = await setup();
    const log = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValue(log.promise);
    let reading!: Promise<void>;
    act(() => { reading = result.current.loadHistoryLog("run"); });
    rerender({ active: true, activeRunId: "run", workspaceId: "w2" });
    rerender({ active: true, activeRunId: "run", workspaceId: "w1" });
    await act(async () => { log.resolve("old lifecycle"); await reading; });
    expect(result.current.historyLogByRun).toEqual({});
  });

  it("ignores retained output callbacks from a previous workspace lifecycle", async () => {
    const { result, rerender } = await setup();
    const oldOutput = result.current;
    rerender({ active: true, activeRunId: "run", workspaceId: "w2" });
    rerender({ active: true, activeRunId: "run", workspaceId: "w1" });
    await act(async () => { await result.current.loadHistoryLog("run"); });
    act(() => listeners[2](event({ data: "current lifecycle" })));
    await tick(16);
    await act(async () => {
      oldOutput.clearTaskOutput("task");
      await oldOutput.loadHistoryLog("old-run");
    });
    expect(readSshTaskRunLog).toHaveBeenCalledExactlyOnceWith("w1", "run");
    expect(result.current.historyLogByRun).toEqual({ run: "history log" });
    expect(result.current.eventsByRun.run[0].data).toBe("current lifecycle");
  });

  it("tracks every concurrent read for one run and retains the first completed log", async () => {
    const { result } = await setup();
    const first = deferred<string>();
    const second = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    let firstRead!: Promise<void>;
    let secondRead!: Promise<void>;
    act(() => {
      firstRead = result.current.loadHistoryLog("run");
      secondRead = result.current.loadHistoryLog("run");
    });
    await act(async () => { second.resolve("first completed"); await secondRead; });
    expect(result.current.historyLogLoading).toBe(true);
    await act(async () => { first.resolve("late response"); await firstRead; });
    expect(result.current.historyLogByRun.run).toBe("first completed");
    expect(result.current.historyLogLoading).toBe(false);
  });

  it("keeps loading tied to the selected run across quick switches and cache hits", async () => {
    const { result, rerender } = await setup();
    await act(async () => { await result.current.loadHistoryLog("cached"); });
    const first = deferred<string>();
    const second = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    let firstRead!: Promise<void>;
    let secondRead!: Promise<void>;
    act(() => { firstRead = result.current.loadHistoryLog("run"); });
    rerender({ active: true, activeRunId: "second", workspaceId: "w1" });
    act(() => { secondRead = result.current.loadHistoryLog("second"); });
    await act(async () => { first.resolve("first log"); await firstRead; });
    expect(result.current.historyLogLoading).toBe(true);
    rerender({ active: true, activeRunId: "cached", workspaceId: "w1" });
    await act(async () => { await result.current.loadHistoryLog("cached"); });
    expect(result.current.historyLogLoading).toBe(false);
    rerender({ active: true, activeRunId: "second", workspaceId: "w1" });
    expect(result.current.historyLogLoading).toBe(true);
    rerender({ active: true, activeRunId: null, workspaceId: "w1" });
    expect(result.current.historyLogLoading).toBe(false);
    await act(async () => { second.resolve("second log"); await secondRead; });
    expect(result.current.historyLogByRun).toEqual({ cached: "history log", run: "first log", second: "second log" });
    expect(readSshTaskRunLog).toHaveBeenCalledTimes(3);
  });

  it("reuses a completed log before React commits the cache update", async () => {
    const { result } = await setup();
    await act(async () => {
      await result.current.loadHistoryLog("run");
      await result.current.loadHistoryLog("run");
    });
    expect(readSshTaskRunLog).toHaveBeenCalledTimes(1);
  });

  it.each(["resolve", "reject"] as const)("ignores a log %s after unmount", async (outcome) => {
    const { result, unmount, handleError } = await setup();
    const log = deferred<string>();
    vi.mocked(readSshTaskRunLog).mockReturnValue(log.promise);
    let reading!: Promise<void>;
    act(() => { reading = result.current.loadHistoryLog("run"); });
    const snapshot = result.current;
    unmount();
    await act(async () => {
      if (outcome === "resolve") log.resolve("unmounted log");
      else log.reject(new Error("unmounted log"));
      await reading;
    });
    expect(result.current).toBe(snapshot);
    expect(handleError).not.toHaveBeenCalled();
    expect(snapshot.historyLogByRun).toEqual({});
  });
});
