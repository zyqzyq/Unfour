// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider, onlineManager } from "@tanstack/react-query";
import { cancelApiRequest, sendApiRequest, type RequestExecutionResult } from "@unfour/command-client";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../model/api-request-tab-state";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSavedApiRequests: vi.fn().mockResolvedValue([]), listApiHistory: vi.fn().mockResolvedValue([]),
  listWorkspaceEnvironments: vi.fn().mockResolvedValue([]), listSshConnections: vi.fn().mockResolvedValue([]),
  getApiRoutes: vi.fn().mockResolvedValue({ request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" }, environmentId: null }),
  listTemporaryRoutes: vi.fn().mockResolvedValue([{ handle: "live-node", displayName: "Node", endpoint: "fixture.invalid:22" }]),
  sendApiRequest: vi.fn(), cancelApiRequest: vi.fn(),
}));
const ended: RequestExecutionResult = {
  response: null, httpError: "SSH_TERMINAL_END_TIMEOUT", httpErrorCode: "SSH_TERMINAL_END_TIMEOUT",
  preRequest: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
  postResponse: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
};
let clients: QueryClient[] = [];
beforeEach(() => onlineManager.setOnline(true));
afterEach(() => { cleanup(); clients.forEach((client) => client.clear()); clients = []; resetApiRequestTabStore(); vi.clearAllMocks(); onlineManager.setOnline(true); });
async function setup(terminal = false) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  clients.push(client);
  const hook = renderHook(() => useApiRequestTabs("w"), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  act(() => hook.result.current.updateDraft("new:1", {
    url: "http://fixture.invalid/embeddings", method: "POST", bodyMode: "raw", rawBodyType: "json", body: '{"input":"synthetic"}',
    ...(terminal ? { temporaryHandle: "live-node", temporaryExecutionMode: "terminalCurl" } : {}),
  }));
  await waitFor(() => expect(hook.result.current.routing.ready).toBe(true));
  if (terminal) act(() => hook.result.current.updateDraft("new:1", { terminalConfirmed: true }));
  return { ...hook, client };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

it.each(["false", "reject"])("cancel %s preserves the pending request and allows another cancellation without resending", async (kind) => {
  const response = deferred<RequestExecutionResult>();
  vi.mocked(sendApiRequest).mockReturnValue(response.promise);
  if (kind === "false") vi.mocked(cancelApiRequest).mockResolvedValue(false);
  else vi.mocked(cancelApiRequest).mockRejectedValue(new Error("synthetic IPC detail must not be displayed"));
  const { result } = await setup();
  await act(async () => result.current.sendTab(result.current.activeTab!));
  const executionId = result.current.activeTab!.executionId;
  await act(async () => result.current.cancelTab(result.current.activeTab!));
  expect(result.current.activeTab).toMatchObject({ sending: true, cancelling: false, executionId });
  expect(result.current.activeTab!.cancelError).toContain("you can try Stop again");
  expect(result.current.activeTab!.cancelError).not.toContain("synthetic IPC");
  vi.mocked(cancelApiRequest).mockResolvedValue(true);
  await act(async () => result.current.cancelTab(result.current.activeTab!));
  expect(result.current.activeTab).toMatchObject({ sending: true, cancelling: true, cancelError: null, executionId });
  expect(cancelApiRequest).toHaveBeenCalledTimes(2);
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  await act(async () => response.resolve(ended));
  await waitFor(() => expect(result.current.activeTab!.sending).toBe(false));
  expect(result.current.activeTab).toMatchObject({ cancelling: false, cancelError: null, sendErrorCode: "SSH_TERMINAL_END_TIMEOUT" });
});

it("does not cancel twice with stale tab objects or apply a late failure to a subsequent send", async () => {
  const first = deferred<RequestExecutionResult>();
  const second = deferred<RequestExecutionResult>();
  const cancellation = deferred<boolean>();
  vi.mocked(sendApiRequest).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  vi.mocked(cancelApiRequest).mockReturnValue(cancellation.promise);
  const { result } = await setup();
  await act(async () => result.current.sendTab(result.current.activeTab!));
  const oldTab = result.current.activeTab!;
  let stopping!: Promise<void>;
  await act(async () => { stopping = result.current.cancelTab(oldTab); await result.current.cancelTab(oldTab); });
  expect(cancelApiRequest).toHaveBeenCalledTimes(1);
  await act(async () => first.resolve(ended));
  await waitFor(() => expect(result.current.activeTab!.sending).toBe(false));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  const executionId = result.current.activeTab!.executionId;
  await act(async () => { cancellation.reject(new Error("late failure")); await stopping; await result.current.cancelTab(oldTab); });
  expect(result.current.activeTab).toMatchObject({ sending: true, cancelling: false, executionId, cancelError: null });
  expect(cancelApiRequest).toHaveBeenCalledTimes(1);
  await act(async () => second.resolve(ended));
});

it("finds a pending cancellation by execution when saving changed the tab ID", async () => {
  const response = deferred<RequestExecutionResult>();
  const cancellation = deferred<boolean>();
  vi.mocked(sendApiRequest).mockReturnValue(response.promise);
  vi.mocked(cancelApiRequest).mockReturnValue(cancellation.promise);
  const { result } = await setup();
  await act(async () => result.current.sendTab(result.current.activeTab!));
  let stopping!: Promise<void>;
  await act(async () => { stopping = result.current.cancelTab(result.current.activeTab!); });
  act(() => {
    const workspace = useApiRequestTabStore.getState().byWorkspace.w;
    useApiRequestTabStore.setState({ byWorkspace: { w: { ...workspace, activeTabId: "saved:request", tabs: workspace.tabs.map((tab) => ({ ...tab, id: "saved:request" })) } } });
  });
  await act(async () => { cancellation.resolve(false); await stopping; });
  expect(result.current.activeTab).toMatchObject({ id: "saved:request", sending: true, cancelling: false });
  expect(result.current.activeTab!.cancelError).toContain("try Stop again");
  await act(async () => response.resolve(ended));
});

it.each([false, true])("dispatches local IPC while browser offline (terminal=%s) and never replays it on reconnect", async (terminal) => {
  onlineManager.setOnline(false);
  vi.mocked(sendApiRequest).mockResolvedValue(ended);
  const { result } = await setup(terminal);
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab!.sending).toBe(false));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  expect(result.current.activeTab!.sendErrorCode).toBe("SSH_TERMINAL_END_TIMEOUT");
  await act(async () => onlineManager.setOnline(true));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
});

it("clears a rejected send even if the API surface unmounted", async () => {
  const response = deferred<RequestExecutionResult>();
  vi.mocked(sendApiRequest).mockReturnValue(response.promise);
  const hook = await setup(true);
  await act(async () => hook.result.current.sendTab(hook.result.current.activeTab!));
  hook.unmount();
  await act(async () => response.reject({ code: "SSH_TERMINAL_STARTUP_TIMEOUT", message: "untrusted detail" }));
  await waitFor(() => expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].sending).toBe(false));
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].sendError).toContain("startup confirmation");
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].sendError).not.toContain("untrusted detail");
});
