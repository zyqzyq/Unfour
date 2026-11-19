// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ApiRoutes, ApiSavedRequest, Workspace, WorkspaceLayout } from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../../api-client/src/model/api-request-tab-state";
import { DesktopApp } from "./DesktopApp";

const { invoke } = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: { workspaceId?: string; layout?: WorkspaceLayout }) => Promise<unknown>>(),
}));
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...await original<typeof import("@tauri-apps/api/core")>(), invoke,
}));
vi.mock("@monaco-editor/react", () => ({ default: () => null }));
vi.mock("@unfour/database", () => ({ DatabasePage: () => null }));
vi.mock("./components/useExternalSshNavigation", () => ({ useExternalSshNavigation: () => undefined }));
vi.mock("./components/useFeatureModulePreload", () => ({ useFeatureModulePreload: () => async () => undefined }));
vi.mock("./components/WindowControls", () => ({ WindowControls: () => null }));

const date = "2026-01-01T00:00:00Z";
const alpha = "ws-alpha";
const beta = "ws-beta";
const workspaces: Workspace[] = [alpha, beta].map((id) => ({
  id, name: id === alpha ? "Alpha" : "Beta", isDefault: id === alpha,
  environmentType: "dev", mcpPolicy: "auto", createdAt: date, updatedAt: date,
  deletedAt: null, lastOpenedAt: null, revision: 1,
}));
const directRoutes: ApiRoutes = {
  request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" }, environmentId: null,
};

function savedRequest(workspaceId: string, name: string, id = `${workspaceId}-req`): ApiSavedRequest {
  return {
    id, workspaceId, name, collectionId: `${workspaceId}-collection`, parentFolderId: null, sortOrder: 0,
    method: "POST", url: `https://example.test/${id}`, headersJson: "[]", queryJson: "[]",
    body: '{"saved":true}', bodyKind: "json", settingsJson: "{}", preRequestScript: "console.log('pre');",
    postResponseScript: "console.log('post');", scriptSchemaVersion: 1, createdAt: date, updatedAt: date,
    deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
  };
}

const alphaRequest = savedRequest(alpha, "Alpha request");
const betaRequest = savedRequest(beta, "Beta request");
const layouts = new Map<string, WorkspaceLayout>();
const readLayout = vi.fn<(workspaceId: string) => Promise<WorkspaceLayout>>();
const readSavedRequests = vi.fn<(workspaceId: string) => Promise<ApiSavedRequest[]>>();
const clients = new Set<QueryClient>();

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => { resolve = res; });
  return { promise, resolve };
}

function persistedLayout(workspaceId: string, requestId: string | null) {
  return { ...useWorkspaceStore.getState().snapshotLayout(workspaceId), selectedApiRequestId: requestId };
}

function resetProcessStores() {
  useWorkspaceStore.setState(useWorkspaceStore.getInitialState(), true);
  resetApiRequestTabStore();
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  window.__TAURI_INTERNALS__ = {};
  let activeWorkspaceId = alpha;
  layouts.set(alpha, persistedLayout(alpha, alphaRequest.id));
  layouts.set(beta, persistedLayout(beta, betaRequest.id));
  readLayout.mockImplementation(async (workspaceId) => structuredClone(layouts.get(workspaceId)!));
  readSavedRequests.mockImplementation(async (workspaceId) => [workspaceId === alpha ? alphaRequest : betaRequest]);
  invoke.mockImplementation(async (command, args) => {
    const workspaceId = args?.workspaceId ?? activeWorkspaceId;
    switch (command) {
      case "workspace_set_active": activeWorkspaceId = workspaceId; return { activeWorkspaceId, workspaces };
      case "workspace_list": return { activeWorkspaceId, workspaces };
      case "workspace_layout_get": return readLayout(workspaceId);
      case "workspace_layout_update": layouts.set(workspaceId, structuredClone(args!.layout!)); return args!.layout;
      case "api_saved_requests": return readSavedRequests(workspaceId);
      case "api_collection_list": return [{ id: `${workspaceId}-collection`, workspaceId, name: "Saved requests" }];
      case "api_routes_get": return directRoutes;
      case "system_health": return { storageReady: true };
      case "frontend_log": return undefined;
      default: return [];
    }
  });
});

afterEach(() => {
  cleanup();
  for (const client of clients) client.clear();
  clients.clear();
  resetProcessStores();
  layouts.clear();
  delete window.__TAURI_INTERNALS__;
  vi.unstubAllGlobals();
  expect(invoke.mock.calls.map(([command]) => command).filter((command) =>
    command.startsWith("api_send_request") || command.startsWith("ssh_task_run") || /script_(execute|run)/.test(command),
  )).toEqual([]);
});

function boot() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  clients.add(client);
  const view = render(<StrictMode><QueryClientProvider client={client}><I18nProvider initialLocale="en">
    <DesktopApp />
  </I18nProvider></QueryClientProvider></StrictMode>);
  return { client, ...view };
}

function requestUrl() {
  return screen.getByRole("textbox", { name: "Request URL" });
}

async function openRequest(request: ApiSavedRequest) {
  fireEvent.click(await screen.findByText(request.name));
  await screen.findByDisplayValue(request.url);
}

async function switchWorkspace(from: string, to: string) {
  fireEvent.keyDown(screen.getByRole("button", { name: new RegExp(`^${from}`) }), { key: "Enter" });
  fireEvent.click(await screen.findByRole("menuitem", { name: new RegExp(`^${to}`) }));
  await screen.findByRole("button", { name: new RegExp(`^${to}`) });
}

it("restores the saved definition after persisting the selection and restarting both stores", async () => {
  layouts.set(alpha, persistedLayout(alpha, null));
  const first = boot();
  await screen.findByRole("textbox", { name: "Request URL" });
  fireEvent.change(requestUrl(), { target: { value: "https://discarded-draft.test" } });
  await openRequest(alphaRequest);
  await waitFor(() => expect(layouts.get(alpha)?.selectedApiRequestId).toBe(alphaRequest.id));

  first.unmount();
  first.client.clear();
  resetProcessStores();
  expect(useApiRequestTabStore.getState().byWorkspace).toEqual({});
  expect(useWorkspaceStore.getState().layoutWorkspaceId).toBeUndefined();
  const { client } = boot();

  const restored = await screen.findByRole("tab", { name: /Alpha request/, selected: true });
  expect(requestUrl()).toHaveValue(alphaRequest.url);
  expect(useApiRequestTabStore.getState().byWorkspace[alpha].tabs).toMatchObject([{
    draft: { body: alphaRequest.body, preRequestScript: alphaRequest.preRequestScript, postResponseScript: alphaRequest.postResponseScript },
    response: null,
  }]);
  expect(readLayout.mock.calls.filter(([id]) => id === alpha)).toHaveLength(2);

  await act(() => client.invalidateQueries({ queryKey: ["api-saved", alpha] }));
  expect(restored).toHaveAttribute("aria-selected", "true");
  fireEvent.click(screen.getByRole("button", { name: "New Request" }));
  await waitFor(() => expect(layouts.get(alpha)?.selectedApiRequestId).toBeNull());
  await act(() => client.invalidateQueries({ queryKey: ["api-saved", alpha] }));
  expect(requestUrl()).toHaveValue("");
  fireEvent.click(restored);
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id);
  fireEvent.click(screen.getByRole("button", { name: "Close Alpha request" }));
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBeNull();
  expect(readSavedRequests.mock.calls.filter(([id]) => id === alpha)).toHaveLength(4);
});

it.each(["layout", "requests"])("waits for both reads in the %s-first order", async (first) => {
  const layout = deferred<WorkspaceLayout>();
  const requests = deferred<ApiSavedRequest[]>();
  readLayout.mockReturnValue(layout.promise);
  readSavedRequests.mockReturnValue(requests.promise);
  const { client } = boot();
  await screen.findByRole("textbox", { name: "Request URL" });

  if (first === "layout") {
    layout.resolve(persistedLayout(alpha, alphaRequest.id));
    await waitFor(() => expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("workspace_layout_update", {
      workspaceId: alpha, layout: expect.objectContaining({ selectedApiRequestId: alphaRequest.id }),
    }));
  } else {
    requests.resolve([alphaRequest]);
    await waitFor(() => expect(client.getQueryState(["api-saved", alpha])?.status).toBe("success"));
    expect(useWorkspaceStore.getState().layoutWorkspaceId).toBeUndefined();
  }
  expect(requestUrl()).toHaveValue("");
  expect(useApiRequestTabStore.getState().byWorkspace[alpha]).toBeUndefined();

  layout.resolve(persistedLayout(alpha, alphaRequest.id));
  requests.resolve([alphaRequest]);
  await screen.findByDisplayValue(alphaRequest.url);
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id);
});

it("waits for the first visit to API when the workspace opens in another module", async () => {
  layouts.set(alpha, { ...persistedLayout(alpha, alphaRequest.id), activeTabId: "database-main" });
  const { client } = boot();
  await screen.findByRole("button", { name: "Database", current: "page" });
  await waitFor(() => expect(client.getQueryState(["api-saved", alpha])?.status).toBe("success"));
  expect(useApiRequestTabStore.getState().byWorkspace[alpha]).toBeUndefined();

  fireEvent.click(screen.getByRole("button", { name: "API Client" }));
  await screen.findByDisplayValue(alphaRequest.url);
});

it.each(["new", "edit"])("keeps a user's %s draft while the saved list is pending", async (action) => {
  const requests = deferred<ApiSavedRequest[]>();
  readSavedRequests.mockReturnValue(requests.promise);
  const { client } = boot();
  await screen.findByRole("textbox", { name: "Request URL" });
  await waitFor(() => expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id));

  if (action === "new") fireEvent.click(screen.getByRole("button", { name: "New Request" }));
  else fireEvent.change(requestUrl(), { target: { value: "https://my-draft.test" } });
  await act(async () => requests.resolve([alphaRequest]));
  await waitFor(() => expect(client.getQueryState(["api-saved", alpha])?.status).toBe("success"));
  expect(requestUrl()).toHaveValue(action === "new" ? "" : "https://my-draft.test");
  expect(screen.queryByRole("tab", { name: /Alpha request/ })).toBeNull();
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBeNull();
});

it("keeps an explicitly opened request when the older layout selection arrives", async () => {
  const layout = deferred<WorkspaceLayout>();
  readLayout.mockReturnValue(layout.promise);
  const other = savedRequest(alpha, "Explicit request", "req-explicit");
  readSavedRequests.mockResolvedValue([alphaRequest, other]);
  boot();
  await openRequest(other);

  layout.resolve(persistedLayout(alpha, alphaRequest.id));
  await waitFor(() => expect(layouts.get(alpha)?.selectedApiRequestId).toBe(other.id));
  expect(requestUrl()).toHaveValue(other.url);
  expect(screen.queryByRole("tab", { name: /Alpha request/ })).toBeNull();
});

it("isolates late reads during a workspace switch and keeps tabs and edits on return", async () => {
  const layout = deferred<WorkspaceLayout>();
  const requests = deferred<ApiSavedRequest[]>();
  readLayout.mockReturnValueOnce(layout.promise);
  readSavedRequests.mockReturnValueOnce(requests.promise);
  const { client } = boot();
  await screen.findByRole("textbox", { name: "Request URL" });
  await switchWorkspace("Alpha", "Beta");
  await screen.findByDisplayValue(betaRequest.url);

  await act(async () => {
    layout.resolve(persistedLayout(alpha, alphaRequest.id));
    requests.resolve([alphaRequest]);
  });
  await waitFor(() => expect(client.getQueryState(["api-saved", alpha])?.status).toBe("success"));
  expect(requestUrl()).toHaveValue(betaRequest.url);
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(betaRequest.id);
  expect(useApiRequestTabStore.getState().byWorkspace[alpha]).toBeUndefined();

  fireEvent.change(requestUrl(), { target: { value: "https://beta-draft.test" } });
  fireEvent.click(screen.getByRole("button", { name: "New Request" }));
  fireEvent.change(requestUrl(), { target: { value: "https://beta-scratch.test" } });
  fireEvent.click(screen.getByRole("button", { name: "Database" }));
  fireEvent.click(screen.getByRole("button", { name: "API Client" }));
  expect(requestUrl()).toHaveValue("https://beta-scratch.test");

  await switchWorkspace("Beta", "Alpha");
  await screen.findByDisplayValue(alphaRequest.url);
  await switchWorkspace("Alpha", "Beta");
  await screen.findByDisplayValue("https://beta-scratch.test");
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: /Beta request/ }));
  expect(requestUrl()).toHaveValue("https://beta-draft.test");
});

it.each(["deleted", "other workspace"])("clears the selection after a successful read confirms it is %s", async (reason) => {
  readSavedRequests.mockResolvedValue(reason === "deleted" ? [] : [{ ...alphaRequest, workspaceId: beta }]);
  boot();
  await waitFor(() => expect(layouts.get(alpha)?.selectedApiRequestId).toBeNull());
  expect(requestUrl()).toHaveValue("");
  expect(screen.queryByRole("tab", { name: /Alpha request/ })).toBeNull();
});

it("retains the saved ID after a list error and restores it after a successful retry", async () => {
  readSavedRequests.mockRejectedValueOnce(new Error("Saved request list unavailable"));
  const { client } = boot();
  await waitFor(() => expect(client.getQueryState(["api-saved", alpha])?.status).toBe("error"));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("workspace_layout_update", {
    workspaceId: alpha, layout: expect.objectContaining({ selectedApiRequestId: alphaRequest.id }),
  }));
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id);
  expect(requestUrl()).toHaveValue("");

  await act(() => client.refetchQueries({ queryKey: ["api-saved", alpha] }));
  await screen.findByDisplayValue(alphaRequest.url);
  expect(useWorkspaceStore.getState().selectedApiRequestId).toBe(alphaRequest.id);
});
