// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { sendApiRequest, type RequestExecutionResult } from "@unfour/command-client";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../model/api-request-tab-state";

const warningMock = vi.hoisted(() => vi.fn());
vi.mock("@unfour/ui", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/ui")>(),
  useFeedback: () => ({ info: warningMock, error: vi.fn(), success: vi.fn(), show: vi.fn() }),
}));
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
  return renderHook(() => useApiRequestTabs("w"), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
}

const skipped = { status: "skipped" as const, console: [], tests: [], durationMs: 0, error: null };
function execution(patch: Partial<RequestExecutionResult> = {}): RequestExecutionResult {
  return {
    response: { historyId: "hist-kept", status: 201, statusText: "Created", headers: [], body: "{\"token\":\"activity-secret-canary\"}", durationMs: 4 },
    httpError: null,
    httpErrorCode: null,
    preRequest: skipped,
    postResponse: skipped,
    ...patch,
  };
}

it("keeps a completed response and shows an activity warning without sending again", async () => {
  vi.mocked(sendApiRequest).mockResolvedValue(execution({ warnings: ["ACTIVITY_RECORD_FAILED"] }));
  const { result } = setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  const tab = useApiRequestTabStore.getState().byWorkspace.w.tabs[0];
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  expect(tab.response?.historyId).toBe("hist-kept");
  expect(tab.response?.status).toBe(201);
  expect(tab.response?.body).toContain("activity-secret-canary");
  expect(tab.sendError).toBeNull();
  expect(warningMock).toHaveBeenCalledTimes(1);
  expect(warningMock.mock.calls[0][0]).toContain("activity log could not be saved");
  expect(warningMock.mock.calls[0][0]).not.toContain("activity-secret-canary");
});

it("does not present http or script failure as an accepted activity warning", async () => {
  vi.mocked(sendApiRequest).mockResolvedValueOnce(execution({
    httpError: "connection refused",
    httpErrorCode: "HTTP_ERROR",
    warnings: ["ACTIVITY_RECORD_FAILED"],
  }));
  const { result } = setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].response?.historyId).toBe("hist-kept");
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].sendError).toContain("connection refused");
  expect(warningMock).not.toHaveBeenCalled();

  warningMock.mockClear();
  vi.mocked(sendApiRequest).mockResolvedValueOnce(execution({
    response: null,
    preRequest: { ...skipped, status: "failed" },
    warnings: [],
  }));
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  expect(sendApiRequest).toHaveBeenCalledTimes(2);
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].response).toBeNull();
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].execution?.preRequest.status).toBe("failed");
  expect(warningMock).not.toHaveBeenCalled();
});
