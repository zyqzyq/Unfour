// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetDatabaseTabStore } from "../model/database-tab-state";
import { buildDatabaseTree, databaseTableTreeId } from "../model/database-tree";
import type { DatabaseConnectionSessionState, DatabaseWorkspaceTab } from "../model/types";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseActiveContext, useDatabaseActiveTableEditing, useDatabaseContextOptions } from "./useDatabaseActiveContext";
import { connection, table } from "./database-controller.test-fixtures";

beforeEach(resetDatabaseTabStore);
afterEach(cleanup);
const pg = connection("pg", "postgres");
const mysql = connection("mysql", "mysql");
const connections = [pg, mysql];
const states: Record<string, DatabaseConnectionSessionState> = { pg: { status: "connected" }, mysql: { status: "failed" } };
const users = table("app", "users", "public");

function contextProps(activeTab: DatabaseWorkspaceTab | null) {
  return { active: true, activeTab, connections, connectionStates: states, selectedConnectionId: "mysql", selectedTable: users };
}

describe("database active context", () => {
  it("uses the table tab for toolbar, structure and editing even when tree selection differs", () => {
    const applyPendingChanges = vi.fn(async () => {});
    const { result, rerender } = renderHook(({ readOnly }) => {
      const tabs = useDatabaseTabs({ workspaceId: "ws" });
      const context = useDatabaseActiveContext({ ...contextProps(tabs.activeTab), connections: [pg, mysql].map((item) => ({ ...item, readOnly })) });
      const editing = useDatabaseActiveTableEditing({
        applyPendingChanges, connection: context.activeTableConnection, connected: context.activeTableStatus === "connected",
        mutationPending: false, tab: context.activeTableTab, updateTableTab: tabs.updateTableTab,
      });
      return { tabs, context, editing };
    }, { initialProps: { readOnly: false } });
    act(() => { result.current.tabs.openTableTab("pg", users, "structure"); });
    const tabId = result.current.tabs.activeTab!.id;
    act(() => result.current.tabs.updateTableTab(tabId, { tableView: { pageIndex: 0, pageSize: 100, readOnly: false, tableName: "users", totalRows: 1 } }));
    expect(result.current.context).toMatchObject({
      activeTableStatus: "connected", selectedConnectionStatus: "failed", schemaEnabled: false,
      structureEnabled: true, toolbarConnectionStatus: "connected", selectedTableId: databaseTableTreeId("mysql", users),
    });
    expect(result.current.context.toolbarConnection?.id).toBe("pg");
    expect(result.current.editing).toMatchObject({ canInsert: true, canUpdateDelete: true, primaryKeyColumns: ["id"] });
    act(() => result.current.editing!.onInsertRow([{ column: "id", mode: "value", value: "2" }]));
    expect(result.current.context.activeTableTab?.pendingChanges).toHaveLength(1);
    const pending = result.current.context.activeTableTab!.pendingChanges;
    let queryId = "";
    act(() => { queryId = result.current.tabs.openQueryTab({ connectionId: "mysql", sql: "select 1" }); });
    expect(result.current.editing).toBeNull();
    expect(result.current.context.toolbarConnection?.id).toBe("mysql");
    act(() => result.current.tabs.setActiveTabId(tabId));
    expect(result.current.editing?.pendingChanges).toBe(pending);
    rerender({ readOnly: true });
    expect(result.current.editing).toBeNull();
    act(() => result.current.tabs.setActiveTabId(queryId));
    expect(result.current.tabs.activeTab).toMatchObject({ sql: "select 1", connectionId: "mysql" });
  });

  it("keeps a query with no connection detached and only falls back to tree selection when there is no tab", () => {
    const query: DatabaseWorkspaceTab = {
      id: "query", kind: "query", connectionId: null, catalog: null, schema: null,
      activeResultIndex: 0, error: null, pendingConfirmation: false, result: null, results: [], resultTab: "results", sql: "", title: "Query",
    };
    const { result, rerender } = renderHook(useDatabaseActiveContext, { initialProps: contextProps(query) });
    expect(result.current.toolbarConnection).toBeNull();
    expect(result.current.toolbarConnectionStatus).toBe("disconnected");
    expect(result.current.activeQueryConnection).toBeUndefined();
    rerender(contextProps(null));
    expect(result.current.toolbarConnection).toBe(mysql);
    expect(result.current.toolbarConnectionStatus).toBe("failed");
  });

  it("merges catalog choices in order and derives schemas only from the selected loaded schema", () => {
    const treeModel = buildDatabaseTree([users, table("app", "events", "audit"), table("extra", "items", "public")]);
    const options: Parameters<typeof useDatabaseContextOptions>[0] = {
      activeQueryConnection: pg, activeQueryTab: null, catalogNames: ["app", "analytics", "", "app"], treeModel,
    };
    const { result, rerender } = renderHook(useDatabaseContextOptions, { initialProps: options });
    expect(result.current.catalogOptions).toEqual(["app", "analytics", "extra"]);
    expect(result.current.schemaOptions).toEqual(["public", "audit"]);
    rerender({ ...options, activeQueryConnection: { ...pg, database: "analytics" } });
    expect(result.current.schemaOptions).toEqual([]);
    rerender({ ...options, treeModel: buildDatabaseTree([table("app")]) });
    expect(result.current.schemaOptions).toEqual([]);
    rerender({ ...options, catalogNames: undefined, treeModel: buildDatabaseTree([table(null)]) });
    expect(result.current.catalogOptions).toEqual([]);
    expect(result.current.schemaOptions).toEqual([]);
  });
});
