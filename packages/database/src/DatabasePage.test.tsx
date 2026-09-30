// @vitest-environment jsdom
import { useCallback, useState, type ComponentProps, type ReactElement, type ReactNode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getDatabaseSchema, listDatabaseCatalogs, listDatabaseConnections, listDatabaseQueryHistory, listSavedSql,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { DatabasePage } from "./DatabasePage";
import { DatabaseSidebar } from "./components/DatabaseSidebar";
import { DatabaseStatusBar } from "./components/DatabaseStatusBar";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { databaseConnectionsQueryKey } from "./hooks/useDatabaseConnections";
import { connection, schema, table } from "./hooks/database-controller.test-fixtures";
import { useDatabaseConnectionStore } from "./model/database-connection-state";
import { resetDatabaseTabStore, useDatabaseTabStore } from "./model/database-tab-state";
import { databaseTableTreeId } from "./model/database-tree";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  getDatabaseSchema: vi.fn(), listDatabaseCatalogs: vi.fn(), listDatabaseConnections: vi.fn(),
  listDatabaseQueryHistory: vi.fn(), listSavedSql: vi.fn(),
}));
// Retain the actual page/controller/tree/slots; the editor surface is covered by
// DatabaseWorkspace tests and would otherwise mount Monaco in this DOM harness.
vi.mock("./components/DatabaseWorkspace", () => ({ DatabaseWorkspace: vi.fn(() => null) }));

const pg = connection("pg", "postgres");
const mysql = connection("mysql", "mysql");
const sqlite = connection("sqlite", "sqlite");
const connections = [pg, mysql, sqlite];
const users = table("app", "users", "public");
const schemaMock = vi.mocked(getDatabaseSchema);
const catalogsMock = vi.mocked(listDatabaseCatalogs);
const tabs = () => useDatabaseTabStore.getState();
const workspace = () => vi.mocked(DatabaseWorkspace).mock.lastCall![0];

beforeEach(() => {
  vi.clearAllMocks();
  resetDatabaseTabStore();
  useWorkspaceStore.setState({ selectedDatabaseConnectionId: "pg" });
  useDatabaseConnectionStore.setState({ byWorkspace: { ws: {
    pg: { status: "connected" }, mysql: { status: "connected" }, sqlite: { status: "connected" },
  } } });
  tabs().openQueryTab("ws", { connectionId: "pg", catalog: "app", schema: "public", sql: "select 1" });
  vi.mocked(listDatabaseConnections).mockResolvedValue(connections);
  vi.mocked(listDatabaseQueryHistory).mockResolvedValue([]);
  vi.mocked(listSavedSql).mockResolvedValue([]);
  catalogsMock.mockImplementation(async (_workspace, id) => id === "pg" ? ["app", "analytics"] : ["app", "archive"]);
  schemaMock.mockImplementation(async (_workspace, id, catalog) => {
    if (id === "sqlite") return schema(id, table(null, "local_items"));
    if (id === "mysql") return schema(id, table(catalog ?? "app", "orders"));
    return schema(id, catalog === "analytics" ? table("analytics", "events", "audit") : users);
  });
});
afterEach(cleanup);

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  client.setQueryData(databaseConnectionsQueryKey("ws"), connections);
  const sidebarChange = vi.fn();
  const statusChange = vi.fn();
  function Shell({ active }: { active: boolean }) {
    const [sidebar, setSidebar] = useState<ReactNode>(null);
    const [status, setStatus] = useState<ReactNode>(null);
    const onSidebar = useCallback((node: ReactNode) => { sidebarChange(node); setSidebar(node); }, []);
    const onStatus = useCallback((node: ReactNode) => { statusChange(node); setStatus(node); }, []);
    return <>
      <DatabasePage active={active} workspaceId="ws" workspaceName="Workspace" onShellSidebarChange={onSidebar} onShellStatusBarChange={onStatus} />
      <aside data-testid="sidebar">{sidebar}</aside>
      <footer data-testid="status">{status}</footer>
    </>;
  }
  const view = render(<QueryClientProvider client={client}><Shell active /></QueryClientProvider>);
  return {
    ...view, client,
    setActive: (active: boolean) => view.rerender(<QueryClientProvider client={client}><Shell active={active} /></QueryClientProvider>),
    sidebar: () => (sidebarChange.mock.lastCall![0] as ReactElement<ComponentProps<typeof DatabaseSidebar>>).props,
    status: () => (statusChange.mock.lastCall![0] as ReactElement<ComponentProps<typeof DatabaseStatusBar>>).props,
  };
}

function treeRow(name: string) {
  return within(screen.getByTestId("sidebar")).getByRole("button", { name }).closest("[role='treeitem']") as HTMLElement;
}
function expand(name: string) {
  const row = treeRow(name);
  if (row.getAttribute("aria-expanded") !== "true") fireEvent.click(within(row).getByRole("button", { name: "Expand" }));
}

describe("DatabasePage structural regression", () => {
  it("keeps multiple roots expanded and only loads a PostgreSQL catalog when expanded", async () => {
    const { sidebar } = setup();
    await waitFor(() => expect(sidebar().catalogNamesByConnection).toEqual({ pg: ["app", "analytics"], mysql: ["app", "archive"] }));
    await waitFor(() => expect(sidebar().schemaCache?.["sqlite::"]).toBeDefined());
    expect(schemaMock).not.toHaveBeenCalledWith("ws", "mysql", null);
    expect(schemaMock).not.toHaveBeenCalledWith("ws", "pg", "analytics");
    expand("mysql");
    expand("sqlite");
    expand("analytics");
    await waitFor(() => expect(sidebar().schemaCache?.["pg::analytics"]).toBeDefined());
    expect(treeRow("pg")).toHaveAttribute("aria-expanded", "true");
    expect(treeRow("mysql")).toHaveAttribute("aria-expanded", "true");
    expect(treeRow("sqlite")).toHaveAttribute("aria-expanded", "true");
    expect(treeRow("analytics")).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(within(treeRow("analytics")).getByRole("button", { name: "Collapse" }));
    expand("analytics");
    expect(schemaMock.mock.calls.filter(([, id, catalog]) => id === "pg" && catalog === "analytics")).toHaveLength(1);
  });

  it("preserves tree caches, query drafts and table changes across connection/tab switches and activation", async () => {
    const { sidebar, status, setActive } = setup();
    await waitFor(() => expect(sidebar().schemaCache?.["pg::app"]).toBeDefined());
    const queryId = workspace().activeTab!.id;
    const originalHandler = sidebar().onNewQuery;
    act(() => sidebar().onToggleCatalog("pg", "analytics"));
    await waitFor(() => expect(sidebar().schemaCache?.["pg::analytics"]).toBeDefined());
    const analytics = sidebar().schemaCache?.["pg::analytics"];
    // Tree selection and toolbar/query context are intentionally independent.
    act(() => sidebar().onSelectConnection(mysql));
    await waitFor(() => expect(sidebar().schemaCache?.["mysql::app"]).toBeDefined());
    expect(status().connection?.id).toBe("pg");
    expect(workspace().activeTab).toMatchObject({ connectionId: "pg", catalog: "app", schema: "public", sql: "select 1" });
    let tableId = "";
    act(() => {
      tableId = tabs().openTableTab("ws", "pg", users);
      tabs().updateTableTab("ws", tableId, { tableView: { pageIndex: 2, pageSize: 100, readOnly: false, tableName: "users", totalRows: 250 } });
    });
    expect(sidebar().selectedTableId).toBe(databaseTableTreeId("pg", users));
    act(() => workspace().tableEditing!.onInsertRow([{ column: "id", mode: "value", value: "2" }]));
    const pending = workspace().tableEditing!.pendingChanges;
    act(() => originalHandler!(mysql, "archive"));
    expect(status().connection?.id).toBe("mysql");
    expect(workspace().activeTab).toMatchObject({ connectionId: "mysql", catalog: "archive" });
    act(() => workspace().onSelectTab(queryId));
    expect(workspace().activeTab).toMatchObject({ sql: "select 1", schema: "public" });
    act(() => workspace().onSelectTab(tableId));
    expect(workspace().tableEditing?.pendingChanges).toBe(pending);
    expect(workspace().activeTab).toMatchObject({ tableView: { pageIndex: 2 } });
    expect(sidebar().schemaCache?.["pg::analytics"]).toBe(analytics);
    expect(sidebar().schemaCache?.["mysql::app"]).toBeDefined();
    setActive(false);
    expect(screen.getByTestId("sidebar")).toBeEmptyDOMElement();
    expect(screen.getByTestId("status")).toBeEmptyDOMElement();
    setActive(true);
    await waitFor(() => expect(sidebar().schemaCache?.["pg::analytics"]).toBe(analytics));
    expect(workspace().tableEditing?.pendingChanges).toBe(pending);
  });

  it("refreshes only the target connection's loaded catalogs and preserves expansion and selection", async () => {
    const { sidebar, client } = setup();
    await waitFor(() => expect(sidebar().schemaCache?.["pg::app"]).toBeDefined());
    expand("analytics");
    await waitFor(() => expect(sidebar().schemaCache?.["pg::analytics"]).toBeDefined());
    act(() => sidebar().onSelectTable("pg", users));
    const selectedId = sidebar().selectedTableId;
    const sqliteCache = sidebar().schemaCache?.["sqlite::"];
    schemaMock.mockClear();
    const invalidate = vi.spyOn(client, "invalidateQueries");
    act(() => sidebar().onRefreshSchema(pg));
    await waitFor(() => expect(schemaMock.mock.calls).toEqual(expect.arrayContaining([
      ["ws", "pg", null], ["ws", "pg", "app"], ["ws", "pg", "analytics"],
    ])));
    await waitFor(() => expect(sidebar().loadingKeys).toEqual([]));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["database-schema", "ws", "pg"] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["database-catalogs", "ws", "pg"] });
    expect(schemaMock.mock.calls.every(([, id]) => id === "pg")).toBe(true);
    expect(sidebar().schemaCache?.["sqlite::"]).toBe(sqliteCache);
    expect(sidebar().selectedTableId).toBe(selectedId);
    expect(treeRow("analytics")).toHaveAttribute("aria-expanded", "true");
  });
});
