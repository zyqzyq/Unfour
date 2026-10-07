// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { sendApiRequest, type RequestExecutionResult } from "@unfour/command-client";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../model/api-request-tab-state";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listSavedApiRequests: vi.fn().mockResolvedValue([]),
  listApiHistory: vi.fn().mockResolvedValue([]),
  listWorkspaceEnvironments: vi.fn().mockResolvedValue([]),
  sendApiRequest: vi.fn(),
}));
afterEach(() => { cleanup(); resetApiRequestTabStore(); vi.clearAllMocks(); });

function setup() {
  useApiRequestTabStore.getState().updateTabDraft("w", "new:1", { url: "https://example.test", method: "POST" });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return renderHook(({ workspaceId }) => useApiRequestTabs(workspaceId), {
    initialProps: { workspaceId: "w" },
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
}
const execution: RequestExecutionResult = {
  response: { historyId: "h", status: 200, statusText: "OK", headers: [], body: "{}", durationMs: 1 },
  httpError: null, httpErrorCode: null,
  preRequest: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
  postResponse: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
};

it("ignores repeated sends, including calls with the same stale tab object", async () => {
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise(() => {}));
  const { result } = setup();
  const tab = result.current.activeTab!;
  await act(async () => { result.current.sendTab(tab); result.current.sendTab(tab); });
  expect(result.current.activeTab!.sending).toBe(true);
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
});

it("applies a response only to the workspace that started the request", async () => {
  let resolve!: (value: RequestExecutionResult) => void;
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise((done) => { resolve = done; }));
  const { result, rerender } = setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  rerender({ workspaceId: "other" });
  await act(async () => { resolve(execution); });
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].response?.status).toBe(200);
  expect(result.current.activeTab!.response).toBeNull();
});

it("discards completion after a tab closes and its id is reused", async () => {
  let resolve!: (value: RequestExecutionResult) => void;
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise((done) => { resolve = done; }));
  const { result } = setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  act(() => { resetApiRequestTabStore("w"); useApiRequestTabStore.getState().updateTabDraft("w", "new:1", { url: "https://new.test" }); });
  await act(async () => { resolve(execution); });
  expect(result.current.activeTab!.response).toBeNull();
});
