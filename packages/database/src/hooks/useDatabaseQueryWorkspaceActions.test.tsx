// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DatabaseConnection, SavedSql } from "@unfour/command-client";
import { resetDatabaseTabStore } from "../model/database-tab-state";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseQueryWorkspaceActions } from "./useDatabaseQueryWorkspaceActions";
import { useQueryHistory } from "./useQueryHistory";
import { useSavedSql } from "./useSavedSql";

vi.mock("@unfour/ui", () => ({ useFeedbackErrorHandler: () => vi.fn() }));
beforeEach(() => resetDatabaseTabStore());
afterEach(cleanup);
const connection: DatabaseConnection = {
  id: "conn", workspaceId: "ws", name: "Server", driver: "postgres", database: "app",
  host: "localhost", port: 5432, username: "user", sslMode: null, sqlitePath: null,
  credentialRef: null, readOnly: false, createdAt: "now", updatedAt: "now",
  deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
};
const saved: SavedSql = {
  id: "saved", workspaceId: "ws", connectionId: "conn", name: "SQL", sql: "select 1",
  catalog: "analytics", schema: "audit", createdAt: "now", updatedAt: "now",
};
function setup() {
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(() => {
    const tabs = useDatabaseTabs({ workspaceId: "ws" });
    const actions = useDatabaseQueryWorkspaceActions({
      activeQueryTab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null,
      activeTableTab: tabs.activeTab?.kind === "table" ? tabs.activeTab : null,
      connections: [connection], databaseTabs: tabs, selectedConnectionId: "conn",
      browseTablePage: vi.fn(), maxHistoryEntries: 100,
      queryHistoryQuery: useQueryHistory("ws", 100, { active: false }),
      savedSqlQuery: useSavedSql("ws", { active: false }),
      setQueryHistory: vi.fn(), setSelectedDatabaseConnection: vi.fn(), setSelectedTable: vi.fn(), t: (key) => key,
    });
    return { tabs, actions };
  }, { wrapper });
}
it("restores identical saved and history execution contexts", () => {
  const { result } = setup();
  act(() => result.current.actions.openSavedSql(saved));
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: "conn", catalog: "analytics", schema: "audit" });
  act(() => result.current.actions.loadHistoryEntry({ ...saved, connectionId: "conn", connectionName: "Server", status: "success", executedAt: "now" }));
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: "conn", catalog: "analytics", schema: "audit" });
});
it("inherits the current query context, honors a database node and resets on connection selection", () => {
  const { result } = setup();
  act(() => result.current.actions.startNewQuery());
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: "conn", catalog: "app" });
  act(() => result.current.actions.openSavedSql(saved));
  act(() => result.current.actions.startNewQuery());
  expect(result.current.tabs.activeTab).toMatchObject({ catalog: "analytics", schema: "audit" });
  act(() => result.current.actions.startNewQuery("conn", "other"));
  expect(result.current.tabs.activeTab).toMatchObject({ catalog: "other", schema: null });
  act(() => result.current.actions.selectQueryConnection("conn"));
  expect(result.current.tabs.activeTab).toMatchObject({ catalog: "app", schema: null });
});
it("retains SQL from unresolved/deleted connections without selecting another connection", () => {
  const { result } = setup();
  act(() => result.current.actions.openSavedSql({ ...saved, connectionId: "gone", catalog: null, schema: null }));
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: null, catalog: null, sql: saved.sql });
});
it("uses the table catalog and schema for generated SQL and New Query", () => {
  const { result } = setup();
  const table = { catalog: "analytics", schema: "audit", name: "users", kind: "table", columns: [] };
  act(() => result.current.actions.loadSqlIntoEditor("conn", "select * from users", table));
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: "conn", catalog: "analytics", schema: "audit" });
  act(() => result.current.tabs.openTableTab("conn", table));
  act(() => result.current.actions.startNewQuery());
  expect(result.current.tabs.activeTab).toMatchObject({ connectionId: "conn", catalog: "analytics", schema: "audit" });
});
