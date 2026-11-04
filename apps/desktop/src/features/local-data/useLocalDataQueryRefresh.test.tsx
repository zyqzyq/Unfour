// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import * as commands from "@unfour/command-client";
import { useLocalDataQueryRefresh } from "./useLocalDataQueryRefresh";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  readLocalDataVersions: vi.fn(),
}));
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });
const version = (value: string, workspaceId = "ws", domain = "environments") => ({ workspaceId, domain, version: value });
async function tick(ms = 1) { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); }
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const read = vi.fn().mockResolvedValue("old");
  const other = vi.fn().mockResolvedValue("api");
  const runtime = vi.fn().mockResolvedValue("runtime");
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const view = renderHook(() => {
    useLocalDataQueryRefresh();
    return {
      environment: useQuery({ queryKey: ["environments", "ws"], meta: { cloudSyncWorkspaceId: "ws", localDataDomains: ["environments"] }, queryFn: read }),
      api: useQuery({ queryKey: ["api", "ws"], meta: { cloudSyncWorkspaceId: "ws", localDataDomains: ["api-requests"] }, queryFn: other }),
      runtime: useQuery({ queryKey: ["runtime", "ws"], queryFn: runtime }),
    };
  }, { wrapper });
  return { ...view, client, read, other, runtime, wrapper };
}
it("polls only affected definition queries; unchanged clocks cause no business reads", async () => {
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("1")]);
  const view = mount(); await tick();
  const count = view.read.mock.calls.length;
  const otherCount = view.other.mock.calls.length;
  await tick(2_000);
  expect(view.read).toHaveBeenCalledTimes(count);
  view.read.mockResolvedValue("MCP changed child only");
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("2")]);
  await tick(2_000);
  expect(view.result.current.environment.data).toBe("MCP changed child only");
  expect(view.read).toHaveBeenCalledTimes(count + 1);
  expect(view.other).toHaveBeenCalledTimes(otherCount);
  expect(view.runtime).toHaveBeenCalledOnce();
  view.unmount(); const polls = vi.mocked(commands.readLocalDataVersions).mock.calls.length;
  await tick(10_000); expect(commands.readLocalDataVersions).toHaveBeenCalledTimes(polls);
});
it("suspends hidden-window polling and catches up immediately on visibility/focus", async () => {
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("1")]);
  const view = mount(); await tick();
  const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  const polls = vi.mocked(commands.readLocalDataVersions).mock.calls.length;
  await tick(8_000); expect(commands.readLocalDataVersions).toHaveBeenCalledTimes(polls);
  view.read.mockResolvedValue("background update");
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("3")]);
  visibility.mockReturnValue("visible");
  act(() => document.dispatchEvent(new Event("visibilitychange"))); await tick();
  expect(view.result.current.environment.data).toBe("background update");
  act(() => window.dispatchEvent(new Event("focus"))); await tick();
  expect(commands.readLocalDataVersions).toHaveBeenCalledTimes(polls + 2);
});
it("keeps its last clock through errors and invalidates inactive workspace caches", async () => {
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("1", "other")]);
  const view = mount(); await tick();
  await view.client.fetchQuery({ queryKey: ["env", "other"], meta: { cloudSyncWorkspaceId: "other", localDataDomains: ["environments"] }, queryFn: async () => "old" });
  vi.mocked(commands.readLocalDataVersions).mockRejectedValueOnce(new Error("database busy"));
  await tick(2_000);
  expect(view.client.getQueryState(["env", "other"])?.isInvalidated).toBe(false);
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([version("2", "other")]);
  await tick(2_000);
  expect(view.client.getQueryState(["env", "other"])?.isInvalidated).toBe(true);
});
it("does not poll a browser preview", async () => {
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue(null);
  const view = mount(); await tick(10_000);
  expect(commands.readLocalDataVersions).toHaveBeenCalledOnce();
  expect(view.read).toHaveBeenCalledOnce();
});
