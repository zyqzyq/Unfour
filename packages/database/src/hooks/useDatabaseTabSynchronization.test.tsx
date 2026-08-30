// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetDatabaseTabStore } from "../model/database-tab-state";
import { buildDatabaseTree } from "../model/database-tree";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseQueryContext, useDatabaseTabSelection } from "./useDatabaseTabSynchronization";

beforeEach(resetDatabaseTabStore);
afterEach(cleanup);

describe("database tab synchronization", () => {
  it("tracks tab connection changes without resetting selection on SQL edits", () => {
    const selectConnection = vi.fn();
    const selectTable = vi.fn();
    const { result } = renderHook(() => {
      const tabs = useDatabaseTabs();
      useDatabaseTabSelection(tabs.activeTab, selectConnection, selectTable);
      return tabs;
    });
    act(() => { result.current.openQueryTab({ connectionId: "conn-a" }); });
    expect(selectConnection).toHaveBeenLastCalledWith("conn-a");
    selectConnection.mockClear();
    selectTable.mockClear();
    act(() => { result.current.updateQueryTab(result.current.activeTab!.id, { sql: "select 1" }); });
    expect(selectConnection).not.toHaveBeenCalled();
    expect(selectTable).not.toHaveBeenCalled();
    act(() => { result.current.updateQueryTab(result.current.activeTab!.id, { connectionId: "conn-b" }); });
    expect(selectConnection).toHaveBeenCalledExactlyOnceWith("conn-b");
  });

  it("normalizes a loaded schema once while preserving explicit catalog and SQL edits", () => {
    const tree = buildDatabaseTree([
      { catalog: "app", schema: "public", name: "users", kind: "table", columns: [] },
      { catalog: "app", schema: "audit", name: "events", kind: "table", columns: [] },
    ]);
    const { result, rerender } = renderHook(({ loaded }) => {
      const tabs = useDatabaseTabs();
      useDatabaseQueryContext(tabs.activeTab, loaded ? tree : null, "app", tabs.updateQueryTab);
      return tabs;
    }, { initialProps: { loaded: false } });
    act(() => { result.current.openQueryTab({ connectionId: "conn", catalog: "app", sql: "select 1" }); });
    rerender({ loaded: true });
    expect(result.current.activeTab).toMatchObject({ catalog: "app", schema: "public", sql: "select 1" });
    act(() => { result.current.updateQueryTab(result.current.activeTab!.id, { schema: "audit", sql: "select 2" }); });
    const tab = result.current.activeTab;
    rerender({ loaded: true });
    expect(result.current.activeTab).toBe(tab);
    expect(tab).toMatchObject({ catalog: "app", schema: "audit", sql: "select 2" });
  });
});
