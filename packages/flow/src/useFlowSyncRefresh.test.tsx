// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import * as commands from "@unfour/command-client";
import { useFlowWorkspaceData } from "./useFlowWorkspaceData";

vi.mock("@unfour/command-client", () => ({
  subscribeCloudSyncWorkspaceChanges: vi.fn(), logFrontendEvent: vi.fn(),
  listFlows: vi.fn(), listSavedApiRequests: vi.fn(), listSshTasks: vi.fn(),
  listDatabaseConnections: vi.fn(), listSshConnections: vi.fn(),
  listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(), getSshTask: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
  for (const command of [commands.listFlows, commands.listSavedApiRequests, commands.listSshTasks,
    commands.listDatabaseConnections, commands.listSshConnections,
    commands.listWorkspaceEnvironments, commands.listWorkspaceVariables]) {
    vi.mocked(command).mockResolvedValue([]);
  }
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return { ...renderHook(() => useFlowWorkspaceData("ws"), { wrapper }), client };
}

it("replaces an in-flight initial read after listener setup and ignores its stale result", async () => {
  let ready!: (stop: () => void) => void;
  let stale!: (flows: commands.FlowDefinition[]) => void;
  const stop = vi.fn();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockImplementation(() => new Promise((resolve) => { ready = resolve; }));
  vi.mocked(commands.listFlows).mockImplementationOnce(() => new Promise((resolve) => { stale = resolve; }));
  const { result, unmount } = mount();
  await act(async () => ready(stop));
  await waitFor(() => expect(result.current.flows.isSuccess).toBe(true));
  expect(commands.listFlows).toHaveBeenCalledTimes(2);
  await act(async () => stale([{ id: "stale" } as commands.FlowDefinition]));
  expect(result.current.flows.data).toEqual([]);
  unmount();
  expect(stop).toHaveBeenCalledOnce();
});

it("refreshes resources and variables only for the matching workspace", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  const { result } = mount();
  await waitFor(() => expect(result.current.resourcesQuery.isSuccess).toBe(true));
  const notify = vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0];
  const reads = vi.mocked(commands.listFlows).mock.calls.length;
  await act(async () => notify("other"));
  expect(commands.listFlows).toHaveBeenCalledTimes(reads);
  vi.mocked(commands.listSavedApiRequests).mockResolvedValue([{ id: "remote-api" } as commands.ApiSavedRequest]);
  vi.mocked(commands.listWorkspaceVariables).mockResolvedValue([{ key: "remote-variable" } as commands.WorkspaceVariable]);
  act(() => notify("ws"));
  await waitFor(() => expect(result.current.resources.api[0]?.id).toBe("remote-api"));
  await waitFor(() => expect(result.current.workspaceVariables.data?.[0]?.key).toBe("remote-variable"));
});

it("unsubscribes a listener that finishes registering after unmount", async () => {
  let ready!: (stop: () => void) => void;
  const stop = vi.fn();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockImplementation(() => new Promise((resolve) => { ready = resolve; }));
  const { unmount } = mount();
  unmount();
  await act(async () => ready(stop));
  expect(stop).toHaveBeenCalledOnce();
});

it("falls back to local reads when event subscription fails and cleans up the timer", async () => {
  vi.useFakeTimers();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockRejectedValue(new Error("unavailable"));
  const { unmount } = mount();
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(commands.logFrontendEvent).toHaveBeenCalledWith(expect.objectContaining({ event: "cloud_sync_listener_failed" }));
  const reads = vi.mocked(commands.listFlows).mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(15_000); });
  expect(commands.listFlows).toHaveBeenCalledTimes(reads + 1);
  unmount();
  const afterUnmount = vi.mocked(commands.listFlows).mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(15_000); });
  expect(commands.listFlows).toHaveBeenCalledTimes(afterUnmount);
});
