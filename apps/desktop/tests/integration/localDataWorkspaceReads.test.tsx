// @vitest-environment jsdom
// Simulated command transport + real query hooks/editor; not a native MCP/WebView test.
import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, afterEach, it, vi, expect } from "vitest";
import type { ReactNode } from "react";
import * as commands from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { WorkspaceEnvironmentsPage } from "../../../../packages/workspace-environments/src/WorkspaceEnvironmentsPage";
import { useSavedSql } from "../../../../packages/database/src/hooks/useSavedSql";
import { useFlowWorkspaceData } from "../../../../packages/flow/src/useFlowWorkspaceData";
import { useLocalDataQueryRefresh } from "../../src/features/local-data/useLocalDataQueryRefresh";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  readLocalDataVersions: vi.fn(), listWorkspaceEnvironments: vi.fn(), listWorkspaceVariables: vi.fn(),
  listSavedSql: vi.fn(), listFlows: vi.fn(), listSavedApiRequests: vi.fn(), listSshTasks: vi.fn(),
  listDatabaseConnections: vi.fn(), listSshConnections: vi.fn(),
  sendApiRequest: vi.fn(), executeDatabaseQuery: vi.fn(), connectSshSession: vi.fn(), runFlow: vi.fn(),
}));
const variable: commands.WorkspaceEnvironmentVariable = {
  id: "v", workspaceId: "ws", environmentId: "env", key: "URL", value: "one", isSecret: false, isEnabled: true,
  description: null, sortOrder: 0, createdAt: "now", updatedAt: "now", revision: 1, deletedAt: null,
};
const environment: commands.WorkspaceEnvironment = {
  id: "env", workspaceId: "ws", name: "Local", isActive: false, sortOrder: 0,
  createdAt: "unchanged", updatedAt: "unchanged", revision: 1, deletedAt: null, variables: [variable],
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([]);
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([environment]);
  for (const read of [commands.listWorkspaceVariables, commands.listSavedSql, commands.listFlows,
    commands.listSavedApiRequests, commands.listSshTasks, commands.listDatabaseConnections, commands.listSshConnections]) vi.mocked(read).mockResolvedValue([]);
});
afterEach(cleanup);
function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  function Listener() { useLocalDataQueryRefresh(); return null; }
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}><I18nProvider initialLocale="en"><Listener />{children}</I18nProvider></QueryClientProvider>;
  return { wrapper, client };
}
async function commit(domain: string, version: number) {
  vi.mocked(commands.readLocalDataVersions).mockResolvedValue([{ workspaceId: "ws", domain, version: String(version) }]);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
}

it("updates, creates and removes child variables with unchanged parent timestamp, retaining dirty edits", async () => {
  const { wrapper, client } = setup();
  render(<WorkspaceEnvironmentsPage workspaceId="ws" onClose={vi.fn()} />, { wrapper });
  fireEvent.click(await screen.findByText("Local"));
  await screen.findByDisplayValue("one");
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ ...environment, variables: [{ ...variable, value: "two", revision: 2 }] }]);
  await commit("environments", 1);
  await screen.findByDisplayValue("two");
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ ...environment, variables: [{ ...variable, value: "two", revision: 2 }, { ...variable, id: "v2", key: "NEW", value: "created" }] }]);
  await commit("environments", 2);
  await screen.findByDisplayValue("created");
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ ...environment, variables: [{ ...variable, value: "two", revision: 2 }] }]);
  await commit("environments", 3);
  await waitFor(() => expect(screen.queryByDisplayValue("created")).not.toBeInTheDocument());
  fireEvent.change(screen.getByDisplayValue("two"), { target: { value: "unsaved" } });
  vi.mocked(commands.listWorkspaceEnvironments).mockResolvedValue([{ ...environment, variables: [{ ...variable, value: "external", revision: 3 }] }]);
  await commit("environments", 4);
  await waitFor(() => expect(client.getQueryData<commands.WorkspaceEnvironment[]>(["workspace-environments", "ws"])?.[0].variables[0].value).toBe("external"));
  expect(screen.getByDisplayValue("unsaved")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.getByRole("button", { name: /latest/i })).toBeInTheDocument();
});

it("refreshes saved SQL and Flow resource dependencies without running any definition", async () => {
  const { wrapper } = setup();
  const { result } = renderHook(() => ({ sql: useSavedSql("ws"), flow: useFlowWorkspaceData("ws") }), { wrapper });
  await waitFor(() => expect(result.current.flow.resourcesQuery.isSuccess).toBe(true));
  const saved: commands.SavedSql = { id: "sql", workspaceId: "ws", connectionId: null, name: "Query", sql: "select 1", catalog: null, createdAt: "now", updatedAt: "now" };
  vi.mocked(commands.listSavedSql).mockResolvedValue([saved]);
  await commit("saved-sql", 1);
  await waitFor(() => expect(result.current.sql.saved).toEqual([saved]));
  vi.mocked(commands.listSavedSql).mockResolvedValue([{ ...saved, sql: "select 2" }]);
  await commit("saved-sql", 2);
  await waitFor(() => expect(result.current.sql.saved[0].sql).toBe("select 2"));
  vi.mocked(commands.listSavedSql).mockResolvedValue([]);
  await commit("saved-sql", 3);
  await waitFor(() => expect(result.current.sql.saved).toEqual([]));
  const reads = vi.mocked(commands.listSavedApiRequests).mock.calls.length;
  await commit("api-requests", 1);
  await waitFor(() => expect(commands.listSavedApiRequests).toHaveBeenCalledTimes(reads + 1));
  for (const run of [commands.sendApiRequest, commands.executeDatabaseQuery, commands.connectSshSession, commands.runFlow]) expect(run).not.toHaveBeenCalled();
});
