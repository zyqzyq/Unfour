// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { getApiRoutes, listSshConnections, sendApiRequest, saveApiRequest, type RequestExecutionResult, type SshConnection } from "@unfour/command-client";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { parseCurl } from "../model/curl-import";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../model/api-request-tab-state";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listSavedApiRequests: vi.fn().mockResolvedValue([]),
  listApiHistory: vi.fn().mockResolvedValue([]),
  listWorkspaceEnvironments: vi.fn().mockResolvedValue([]),
  getApiRoutes: vi.fn().mockResolvedValue({ request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" }, environmentId: null }),
  listSshConnections: vi.fn().mockResolvedValue([]),
  sendApiRequest: vi.fn(),
  saveApiRequest: vi.fn(),
}));
afterEach(() => { cleanup(); resetApiRequestTabStore(); vi.clearAllMocks(); });

async function setup() {
  useApiRequestTabStore.getState().updateTabDraft("w", "new:1", { url: "https://example.test", method: "POST" });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const hook = renderHook(({ workspaceId }) => useApiRequestTabs(workspaceId), {
    initialProps: { workspaceId: "w" },
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  await waitFor(() => expect(hook.result.current.routing.ready).toBe(true));
  return { ...hook, client };
}
const execution: RequestExecutionResult = {
  response: { historyId: "h", status: 200, statusText: "OK", headers: [], body: "{}", durationMs: 1 },
  httpError: null, httpErrorCode: null,
  preRequest: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
  postResponse: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
};

it("opens an imported draft without sending, saving or executing scripts", async () => {
  const { result } = await setup();
  await act(async () => { result.current.newRequest(parseCurl("curl https://example.test -d 'a=1'")); });
  expect(result.current.activeTab).toMatchObject({ savedRequestId: null, source: "new", sending: false, saving: false, execution: null });
  expect(result.current.activeTab!.draft).toMatchObject({ body: "a=1", preRequestScript: "", postResponseScript: "" });
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(saveApiRequest).not.toHaveBeenCalled();
});

it("blocks both sending and saving a history snapshot until reviewed and repaired", async () => {
  const { result } = await setup();
  act(() => {
    const workspace = useApiRequestTabStore.getState().byWorkspace.w;
    useApiRequestTabStore.setState({ byWorkspace: { w: { ...workspace, tabs: workspace.tabs.map((tab) => ({ ...tab, source: "history" })) } } });
  });
  await act(async () => {
    result.current.sendTab(result.current.activeTab!);
    await result.current.saveTab(result.current.activeTab!);
  });
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(saveApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab!.saveError).toContain("Review Auth");
  act(() => result.current.updateDraft("new:1", { historyReviewConfirmed: true, headers: [{ key: "Authorization", value: "<redacted>", enabled: true }] }));
  await act(async () => {
    result.current.sendTab(result.current.activeTab!);
    await result.current.saveTab(result.current.activeTab!);
  });
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(saveApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab!.saveError).toContain("Replace or remove");
});

it("ignores repeated sends, including calls with the same stale tab object", async () => {
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise(() => {}));
  const { result } = await setup();
  const tab = result.current.activeTab!;
  await act(async () => { result.current.sendTab(tab); result.current.sendTab(tab); });
  expect(result.current.activeTab!.sending).toBe(true);
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
});

it("pins the effective SSH host revision and distinguishes a draft override from the stored policy", async () => {
  const route = { mode: "ssh" as const, sshConnectionId: "jump" };
  vi.mocked(getApiRoutes).mockResolvedValueOnce({ request: route, environment: { mode: "direct" }, effective: route, environmentId: null });
  vi.mocked(listSshConnections).mockResolvedValueOnce([{ id: "jump", name: "Bastion", revision: 7 } as SshConnection]);
  vi.mocked(sendApiRequest).mockResolvedValue(execution);
  const { result } = await setup();
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(vi.mocked(sendApiRequest).mock.lastCall?.[2]).toEqual({
    expectedEffective: route, expectedEnvironmentId: null, expectedSshRevision: 7, requestId: null, route: undefined,
  });
  act(() => result.current.updateDraft("new:1", { route: { expected: { mode: "inherit" }, route: { mode: "direct" } } }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(vi.mocked(sendApiRequest).mock.lastCall?.[2]).toEqual({
    expectedEffective: { mode: "direct" }, expectedEnvironmentId: null, expectedSshRevision: undefined, requestId: null, route: { mode: "direct" },
  });
});

it("gates the actual Send handler synchronously during activation and permits retry after failure", async () => {
  vi.mocked(sendApiRequest).mockResolvedValue(execution);
  const { result, client } = await setup();
  const send = result.current.sendTab;
  const tab = result.current.activeTab!;
  let reject!: (error: Error) => void;
  const mutation = client.getMutationCache().build(client, {
    mutationKey: ["workspace-environment-activation", "w"],
    mutationFn: () => new Promise<void>((_resolve, fail) => { reject = fail; }),
  });
  let pending!: Promise<unknown>;
  await act(async () => {
    pending = mutation.execute(undefined).catch(() => undefined);
    send(tab);
  });
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab!.sending).toBe(false);
  await act(async () => { reject(new Error("isolated activation failure")); await pending; });
  await act(async () => send(tab));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  expect(result.current.activeTab!.response?.status).toBe(200);
});

it("does not block sends in another workspace or change a running request when activation starts", async () => {
  let finish!: (value: RequestExecutionResult) => void;
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const { result, client } = await setup();
  const mutation = client.getMutationCache().build(client, {
    mutationKey: ["workspace-environment-activation", "other"], mutationFn: () => new Promise<void>(() => {}),
  });
  await act(async () => { void mutation.execute(undefined); result.current.sendTab(result.current.activeTab!); });
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  const input = vi.mocked(sendApiRequest).mock.calls[0][1];
  const ownMutation = client.getMutationCache().build(client, {
    mutationKey: ["workspace-environment-activation", "w"], mutationFn: () => new Promise<void>(() => {}),
  });
  await act(async () => { void ownMutation.execute(undefined); finish(execution); });
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  expect(input.workspaceId).toBe("w");
  expect(result.current.activeTab!.response?.status).toBe(200);
  client.clear();
});

it("applies a response only to the workspace that started the request", async () => {
  let resolve!: (value: RequestExecutionResult) => void;
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise((done) => { resolve = done; }));
  const { result, rerender } = await setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  rerender({ workspaceId: "other" });
  await act(async () => { resolve(execution); });
  expect(useApiRequestTabStore.getState().byWorkspace.w.tabs[0].response?.status).toBe(200);
  expect(result.current.activeTab!.response).toBeNull();
});

it("discards completion after a tab closes and its id is reused", async () => {
  let resolve!: (value: RequestExecutionResult) => void;
  vi.mocked(sendApiRequest).mockImplementation(() => new Promise((done) => { resolve = done; }));
  const { result } = await setup();
  await act(async () => { result.current.sendTab(result.current.activeTab!); });
  act(() => { resetApiRequestTabStore("w"); useApiRequestTabStore.getState().updateTabDraft("w", "new:1", { url: "https://new.test" }); });
  await act(async () => { resolve(execution); });
  expect(result.current.activeTab!.response).toBeNull();
});
