// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import * as commands from "@unfour/command-client";
import { useCloudSyncQueryRefresh } from "./useCloudSyncQueryRefresh";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  subscribeCloudSyncWorkspaceChanges: vi.fn(), logFrontendEvent: vi.fn(),
}));
beforeEach(() => vi.resetAllMocks());
afterEach(() => { cleanup(); vi.useRealTimers(); });

function mount(read = vi.fn<() => Promise<string>>().mockResolvedValue("fresh")) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const view = renderHook(() => {
    useCloudSyncQueryRefresh();
    return useQuery({ queryKey: ["local-read", "ws"], meta: { cloudSyncWorkspaceId: "ws" }, queryFn: read });
  }, { wrapper });
  return { ...view, client, wrapper, read };
}
function notify(workspaceId = "ws") {
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0](workspaceId));
}

it("rereads after a pre-commit initial read finishes during listener setup", async () => {
  let ready!: (stop: () => void) => void;
  let stale!: (value: string) => void;
  const stop = vi.fn();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockImplementation(() => new Promise((resolve) => { ready = resolve; }));
  const read = vi.fn<() => Promise<string>>().mockResolvedValue("fresh")
    .mockImplementationOnce(() => new Promise((resolve) => { stale = resolve; }));
  const { result, unmount } = mount(read);
  await act(async () => ready(stop));
  expect(read).toHaveBeenCalledOnce();
  await act(async () => stale("stale"));
  await waitFor(() => expect(result.current.data).toBe("fresh"));
  unmount();
  expect(stop).toHaveBeenCalledOnce();
});

it("refreshes matching active queries and the global workspace list, leaving runtime data alone", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  const { wrapper, read } = mount();
  const globalRead = vi.fn().mockResolvedValue("global");
  const otherRead = vi.fn().mockResolvedValue("other");
  const runtimeRead = vi.fn().mockResolvedValue("runtime");
  const { result } = renderHook(() => ({
    global: useQuery({ queryKey: ["workspaces"], meta: { cloudSyncWorkspaceId: null }, queryFn: globalRead }),
    other: useQuery({ queryKey: ["local-read", "other"], meta: { cloudSyncWorkspaceId: "other" }, queryFn: otherRead }),
    runtime: useQuery({ queryKey: ["runtime", "ws"], queryFn: runtimeRead }),
  }), { wrapper });
  await waitFor(() => expect(result.current.runtime.isSuccess && result.current.global.isSuccess && result.current.other.isSuccess).toBe(true));
  const count = read.mock.calls.length;
  const globalCount = globalRead.mock.calls.length;
  const otherCount = otherRead.mock.calls.length;
  notify();
  await waitFor(() => expect(read).toHaveBeenCalledTimes(count + 1));
  expect(globalRead).toHaveBeenCalledTimes(globalCount + 1);
  expect(otherRead).toHaveBeenCalledTimes(otherCount);
  expect(runtimeRead).toHaveBeenCalledOnce();
  expect(commands.subscribeCloudSyncWorkspaceChanges).toHaveBeenCalledOnce();
});

it("invalidates cached data in a hidden workspace without fetching until its query is enabled", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  const { client, wrapper, result } = mount();
  await waitFor(() => expect(result.current.isSuccess && !result.current.isFetching).toBe(true));
  const read = vi.fn().mockResolvedValue("new value");
  await client.fetchQuery({ queryKey: ["hidden", "other"], meta: { cloudSyncWorkspaceId: "other" }, queryFn: async () => "old value" });
  const view = renderHook(({ active }) => useQuery({
    queryKey: ["hidden", "other"], meta: { cloudSyncWorkspaceId: "other" }, queryFn: read, enabled: active,
  }), { wrapper, initialProps: { active: false } });
  notify("other");
  await waitFor(() => expect(client.getQueryState(["hidden", "other"])?.isInvalidated).toBe(true));
  expect(read).not.toHaveBeenCalled();
  view.rerender({ active: true });
  await waitFor(() => expect(view.result.current.data).toBe("new value"));
});

it("queues a fresh read when another commit arrives during an in-flight refetch", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  const { result, read } = mount();
  await waitFor(() => expect(result.current.data).toBe("fresh"));
  let stale!: (value: string) => void;
  read.mockImplementationOnce(() => new Promise((resolve) => { stale = resolve; }));
  notify();
  await waitFor(() => expect(stale).toBeDefined());
  read.mockResolvedValue("latest");
  notify();
  await act(async () => stale("stale"));
  await waitFor(() => expect(result.current.data).toBe("latest"));
});

it("does not cancel an imperative detail fetch and leaves its cache stale for the next request", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  const { client, result } = mount();
  await waitFor(() => expect(result.current.isSuccess && !result.current.isFetching).toBe(true));
  let finish!: (value: string) => void;
  const pending = client.fetchQuery({
    queryKey: ["detail", "ws"], meta: { cloudSyncWorkspaceId: "ws" },
    queryFn: () => new Promise<string>((resolve) => { finish = resolve; }),
  });
  notify();
  await act(async () => finish("old detail"));
  await expect(pending).resolves.toBe("old detail");
  await waitFor(() => expect(client.getQueryState(["detail", "ws"])?.isInvalidated).toBe(true));
  const latest = await client.fetchQuery({
    queryKey: ["detail", "ws"], meta: { cloudSyncWorkspaceId: "ws" }, queryFn: async () => "latest detail",
  });
  expect(latest).toBe("latest detail");
});

it("unsubscribes a listener that becomes ready after unmount", async () => {
  let ready!: (stop: () => void) => void;
  const stop = vi.fn();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockImplementation(() => new Promise((resolve) => { ready = resolve; }));
  const { unmount } = mount();
  unmount();
  await act(async () => ready(stop));
  expect(stop).toHaveBeenCalledOnce();
});

it("does not perform native startup rereads in a browser preview", async () => {
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(null);
  const { result, read } = mount();
  await waitFor(() => expect(result.current.isSuccess).toBe(true));
  expect(read).toHaveBeenCalledOnce();
});

it("falls back to local reads after subscription failure and disposes the timer", async () => {
  vi.useFakeTimers();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockRejectedValue(new Error("unavailable"));
  const { read, unmount } = mount();
  await act(async () => { await vi.advanceTimersByTimeAsync(1); });
  expect(commands.logFrontendEvent).toHaveBeenCalledWith(expect.objectContaining({ event: "cloud_sync_listener_failed" }));
  const count = read.mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(15_000); });
  expect(read).toHaveBeenCalledTimes(count + 1);
  unmount();
  const finalCount = read.mock.calls.length;
  await act(async () => { await vi.advanceTimersByTimeAsync(15_000); });
  expect(read).toHaveBeenCalledTimes(finalCount);
});
