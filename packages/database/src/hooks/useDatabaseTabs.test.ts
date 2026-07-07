// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { DatabaseTable } from "@unfour/command-client";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { resetDatabaseTabStore } from "../model/database-tab-state";

function table(name: string, patch: Partial<DatabaseTable> = {}): DatabaseTable {
  return {
    catalog: "app",
    schema: "public",
    name,
    kind: "table",
    columns: [],
    ...patch,
  };
}

describe("useDatabaseTabs", () => {
  // The tab state now lives in a module-level store, so it persists across
  // hook instances. Reset it between cases so each test starts clean.
  beforeEach(() => {
    resetDatabaseTabStore();
  });

  it("opens with Query 1 as the active tab", () => {
    const { result } = renderHook(() => useDatabaseTabs());

    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.activeTab?.kind).toBe("query");
    expect(result.current.activeTab?.title).toBe("Query 1");
  });

  it("creates unique query tabs with optional connection and SQL context", () => {
    const { result } = renderHook(() => useDatabaseTabs());

    act(() => {
      result.current.openQueryTab({
        connectionId: "conn-1",
        catalog: "app",
        schema: "public",
        sql: "select * from users;",
      });
    });

    expect(result.current.tabs.map((tab) => tab.title)).toEqual(["Query 1", "Query 2"]);
    expect(result.current.activeTab).toMatchObject({
      kind: "query",
      connectionId: "conn-1",
      catalog: "app",
      schema: "public",
      sql: "select * from users;",
      title: "Query 2",
    });
  });

  it("reuses an existing table tab for the same table and creates a tab for a different table", () => {
    const users = table("users");
    const orders = table("orders");
    const { result } = renderHook(() => useDatabaseTabs());

    let firstId = "";
    act(() => {
      firstId = result.current.openTableTab("conn-1", users, "data");
    });
    act(() => {
      result.current.openTableTab("conn-1", users, "structure");
    });
    act(() => {
      result.current.openTableTab("conn-1", orders, "data");
    });

    expect(result.current.tabs.filter((tab) => tab.kind === "table")).toHaveLength(2);
    expect(result.current.tabs.find((tab) => tab.id === firstId)).toMatchObject({
      kind: "table",
      segment: "structure",
      table: users,
    });
    expect(result.current.activeTab).toMatchObject({
      kind: "table",
      table: orders,
    });
  });

  it("keeps query and table state isolated by tab", () => {
    const users = table("users");
    const { result } = renderHook(() => useDatabaseTabs());
    const queryOneId = result.current.activeTabId;

    let queryTwoId = "";
    let tableId = "";
    act(() => {
      queryTwoId = result.current.openQueryTab({ sql: "select 1;" });
    });
    act(() => {
      tableId = result.current.openTableTab("conn-1", users, "data");
    });
    act(() => {
      result.current.updateQueryTab(queryOneId, { sql: "select * from users;" });
      result.current.updateQueryTab(queryTwoId, { resultTab: "history" });
      result.current.updateTableTab(tableId, {
        tableQuery: { orderBy: "id", orderDescending: true, filter: "active" },
      });
    });

    expect(result.current.tabs.find((tab) => tab.id === queryOneId)).toMatchObject({
      kind: "query",
      sql: "select * from users;",
      resultTab: "results",
    });
    expect(result.current.tabs.find((tab) => tab.id === queryTwoId)).toMatchObject({
      kind: "query",
      sql: "select 1;",
      resultTab: "history",
    });
    expect(result.current.tabs.find((tab) => tab.id === tableId)).toMatchObject({
      kind: "table",
      tableQuery: { orderBy: "id", orderDescending: true, filter: "active" },
    });
  });

  it("keeps an empty query tab when the last tab closes", () => {
    const { result } = renderHook(() => useDatabaseTabs());

    act(() => {
      result.current.closeTab(result.current.activeTabId);
    });

    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.activeTab).toMatchObject({ kind: "query", sql: "" });
  });

  it("leaves a single sequential Query 2 tab when the last tab closes", () => {
    const { result } = renderHook(() => useDatabaseTabs());

    act(() => {
      result.current.closeTab(result.current.activeTabId);
    });

    // When the only tab is closed, a fresh replacement is auto-created. It must
    // keep the next sequential index ("Query 2", not a skipped number). The
    // index is advanced outside the setState updater so it stays correct even
    // under React.StrictMode (which double-invokes updaters in dev and used to
    // skip the index, e.g. producing "Query 3").
    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.tabs[0].id).toBe("database-query-2");
    expect(result.current.tabs[0].title).toBe("Query 2");
  });

  it("removes table tabs and unbinds query tabs when a connection is deleted", () => {
    const { result } = renderHook(() => useDatabaseTabs());

    act(() => {
      result.current.updateQueryTab(result.current.activeTabId, {
        connectionId: "conn-1",
        catalog: "app",
        schema: "public",
        sql: "select 1;",
      });
      result.current.openTableTab("conn-1", table("users"), "data");
      result.current.openQueryTab({ connectionId: "conn-2", sql: "select 2;" });
    });
    act(() => {
      result.current.removeConnectionTabs("conn-1");
    });

    expect(result.current.tabs.some((tab) => tab.kind === "table")).toBe(false);
    expect(result.current.tabs.find((tab) => tab.id.endsWith("query-1"))).toMatchObject({
      kind: "query",
      connectionId: null,
      catalog: null,
      schema: null,
      sql: "select 1;",
    });
    expect(result.current.tabs.find((tab) => tab.id.endsWith("query-2"))).toMatchObject({
      kind: "query",
      connectionId: "conn-2",
    });
  });

  it("keeps tabs isolated per workspace", () => {
    const wsA = renderHook(() => useDatabaseTabs({ workspaceId: "workspace-a" }));
    const wsB = renderHook(() => useDatabaseTabs({ workspaceId: "workspace-b" }));

    act(() => {
      wsA.result.current.openQueryTab({ sql: "select a;" });
    });
    act(() => {
      wsB.result.current.openQueryTab({ sql: "select b;" });
    });

    expect(wsA.result.current.tabs.map((tab) => (tab.kind === "query" ? tab.sql : null))).toEqual([
      "",
      "select a;",
    ]);
    expect(wsB.result.current.tabs.map((tab) => (tab.kind === "query" ? tab.sql : null))).toEqual([
      "",
      "select b;",
    ]);
    expect(wsA.result.current.activeTab).toMatchObject({ kind: "query", sql: "select a;" });
    expect(wsB.result.current.activeTab).toMatchObject({ kind: "query", sql: "select b;" });
  });
});
