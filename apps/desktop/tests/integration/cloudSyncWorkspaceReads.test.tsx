// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import * as commands from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { useCloudSyncQueryRefresh } from "../../src/features/cloud-sync/useCloudSyncQueryRefresh";
import { useApiRequestTabs } from "../../../../packages/api-client/src/hooks/useApiRequestTabs";
import { useApiCollections } from "../../../../packages/api-client/src/hooks/useApiCollections";
import { useApiCollectionFolders } from "../../../../packages/api-client/src/hooks/useApiCollectionFolders";
import { useApiCollectionTreeData } from "../../../../packages/api-client/src/hooks/useApiCollectionTreeData";
import { resetApiRequestTabStore } from "../../../../packages/api-client/src/model/api-request-tab-state";
import { useDatabaseConnections } from "../../../../packages/database/src/hooks/useDatabaseConnections";
import { useDatabaseConnectionForm } from "../../../../packages/database/src/hooks/useDatabaseConnectionForm";
import { connection } from "../../../../packages/database/src/hooks/database-controller.test-fixtures";
import { resetDatabaseTabStore, useDatabaseTabStore } from "../../../../packages/database/src/model/database-tab-state";
import { useSshConnections } from "../../../../packages/ssh-terminal/src/hooks/useSshConnections";
import { WorkspaceEnvironmentsPage } from "../../../../packages/workspace-environments/src/WorkspaceEnvironmentsPage";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  subscribeCloudSyncWorkspaceChanges: vi.fn(), listSavedApiRequests: vi.fn(), listApiHistory: vi.fn(),
  listApiCollections: vi.fn(), listApiCollectionFolders: vi.fn(), listDatabaseConnections: vi.fn(),
  listSshConnections: vi.fn(), listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(),
  executeDatabaseQuery: vi.fn(), sendApiRequest: vi.fn(), connectSshSession: vi.fn(),
}));
const request: commands.ApiSavedRequest = {
  id: "request", workspaceId: "ws", name: "Remote request", collectionId: "collection", parentFolderId: "folder",
  sortOrder: 0, method: "GET", url: "https://example.test", headersJson: "[]", queryJson: "[]", body: null,
  bodyKind: "none", settingsJson: "{}", preRequestScript: null, postResponseScript: null, scriptSchemaVersion: 1,
  createdAt: "now", updatedAt: "now", deletedAt: null, revision: 1, syncStatus: "synced", remoteId: null,
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mockResolvedValue(vi.fn());
  for (const command of [commands.listSavedApiRequests, commands.listApiHistory, commands.listApiCollections,
    commands.listApiCollectionFolders, commands.listDatabaseConnections, commands.listSshConnections,
    commands.listWorkspaceEnvironments, commands.listWorkspaceVariables]) vi.mocked(command).mockResolvedValue([]);
});
afterEach(() => { cleanup(); resetApiRequestTabStore(); resetDatabaseTabStore(); });
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  function Listener() { useCloudSyncQueryRefresh(); return null; }
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>
    <I18nProvider initialLocale="en"><Listener />{children}</I18nProvider>
  </QueryClientProvider>;
  return { client, wrapper };
}
function notify() {
  act(() => vi.mocked(commands.subscribeCloudSyncWorkspaceChanges).mock.calls[0][0]("ws"));
}

it("refreshes API collections, folders and both request-list observers while retaining a draft", async () => {
  const { wrapper } = setup();
  const { result } = renderHook(() => ({
    api: useApiRequestTabs("ws"), collections: useApiCollections("ws"), folders: useApiCollectionFolders("ws"),
    tree: useApiCollectionTreeData("ws", "", []),
  }), { wrapper });
  await waitFor(() => expect(result.current.api.savedRequests).toEqual([]));
  act(() => result.current.api.updateDraft("new:1", { url: "https://unsaved.test", body: "unsaved body" }));
  vi.mocked(commands.listSavedApiRequests).mockResolvedValue([request]);
  vi.mocked(commands.listApiCollections).mockResolvedValue([{ id: "collection", workspaceId: "ws", name: "Remote collection", description: null, createdAt: "now", updatedAt: "now" }]);
  vi.mocked(commands.listApiCollectionFolders).mockResolvedValue([{ id: "folder", workspaceId: "ws", collectionId: "collection", parentFolderId: null, name: "Remote folder", sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null }]);
  notify();
  await waitFor(() => expect(result.current.api.savedRequests[0]?.name).toBe("Remote request"));
  expect(result.current.tree.requests[0]?.id).toBe("request");
  expect(result.current.collections.collections[0]?.name).toBe("Remote collection");
  expect(result.current.folders.folders[0]?.name).toBe("Remote folder");
  expect(result.current.api.activeTab?.draft).toMatchObject({ url: "https://unsaved.test", body: "unsaved body" });
  vi.mocked(commands.listSavedApiRequests).mockResolvedValue([]);
  notify();
  await waitFor(() => expect(result.current.tree.requests).toEqual([]));
  expect(result.current.api.activeTab?.draft.url).toBe("https://unsaved.test");
  expect(commands.sendApiRequest).not.toHaveBeenCalled();
});

it("refreshes database and SSH connections without changing connection edits or SQL drafts", async () => {
  const original = connection("db", "postgres");
  vi.mocked(commands.listDatabaseConnections).mockResolvedValue([original]);
  const tabs = useDatabaseTabStore.getState();
  const tabId = tabs.openQueryTab("ws", { connectionId: "db", sql: "select unsaved_value" });
  const { wrapper } = setup();
  const { result } = renderHook(() => {
    const database = useDatabaseConnections("ws");
    const ssh = useSshConnections("ws");
    const form = useDatabaseConnectionForm("ws", "db", database.data?.[0] ?? null);
    return { database, ssh, form };
  }, { wrapper });
  await waitFor(() => expect(result.current.database.data?.[0]?.name).toBe("db"));
  act(() => result.current.form.setForm((form) => ({ ...form, name: "Unsaved connection", host: "draft.test" })));
  vi.mocked(commands.listDatabaseConnections).mockResolvedValue([{ ...original, name: "Remote database", revision: 2 }]);
  vi.mocked(commands.listSshConnections).mockResolvedValue([{
    id: "ssh", workspaceId: "ws", name: "Remote SSH", host: "remote.test", port: 22, username: "user",
    authKind: "password", keyPath: null, credentialRef: null, createdAt: "now", updatedAt: "now", deletedAt: null,
    revision: 1, syncStatus: "synced", remoteId: null,
  }]);
  notify();
  await waitFor(() => expect(result.current.database.data?.[0]?.name).toBe("Remote database"));
  await waitFor(() => expect(result.current.ssh.data?.[0]?.name).toBe("Remote SSH"));
  expect(result.current.form.form).toMatchObject({ name: "Unsaved connection", host: "draft.test" });
  expect(useDatabaseTabStore.getState().byWorkspace.ws.tabs.find((tab) => tab.id === tabId)).toMatchObject({ sql: "select unsaved_value" });
  vi.mocked(commands.listDatabaseConnections).mockResolvedValue([]);
  vi.mocked(commands.listSshConnections).mockResolvedValue([]);
  notify();
  await waitFor(() => expect(result.current.database.data).toEqual([]));
  await waitFor(() => expect(result.current.ssh.data).toEqual([]));
  expect(commands.executeDatabaseQuery).not.toHaveBeenCalled();
  expect(commands.connectSshSession).not.toHaveBeenCalled();
});

it("updates open workspace variables and environment names, preserving a dirty variable value", async () => {
  const variable: commands.WorkspaceVariable = {
    id: "variable", workspaceId: "ws", key: "base_url", value: "remote value", isSecret: false, isEnabled: true,
    description: null, sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null, revision: 1,
  };
  const { wrapper, client } = setup();
  render(<WorkspaceEnvironmentsPage workspaceId="ws" onClose={vi.fn()} />, { wrapper });
  await screen.findByRole("heading", { name: "Workspace Variables" });
  vi.mocked(commands.listWorkspaceVariables).mockResolvedValue([variable]);
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{
    id: "environment", workspaceId: "ws", name: "Remote environment", sortOrder: 0, variables: [], isActive: false,
    createdAt: "now", updatedAt: "now", deletedAt: null, revision: 1,
  }]);
  notify();
  const value = await screen.findByDisplayValue("remote value");
  await screen.findByText("Remote environment");
  fireEvent.change(value, { target: { value: "unsaved value" } });
  vi.mocked(commands.listWorkspaceVariables).mockResolvedValue([{ ...variable, value: "new server value", revision: 2 }]);
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([]);
  notify();
  await waitFor(() => expect(client.getQueryData<commands.WorkspaceVariable[]>(["workspace-variables", "ws"])?.[0]?.value).toBe("new server value"));
  await waitFor(() => expect(screen.queryByText("Remote environment")).not.toBeInTheDocument());
  expect(screen.getByDisplayValue("unsaved value")).toBeInTheDocument();
});
