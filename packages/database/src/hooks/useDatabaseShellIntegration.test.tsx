// @vitest-environment jsdom
import type { ComponentProps, ReactElement } from "react";
import { cleanup, fireEvent, render, renderHook, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DatabaseSidebar } from "../components/DatabaseSidebar";
import { DatabaseStatusBar } from "../components/DatabaseStatusBar";
import type { DatabaseSidebarActions } from "../model/database-page";
import { useDatabaseShellIntegration } from "./useDatabaseShellIntegration";
import { connection, schema, table } from "./database-controller.test-fixtures";

afterEach(cleanup);
const pg = connection("pg", "postgres");
const users = table("app");
const saved = {
  id: "saved", workspaceId: "ws", connectionId: "pg", name: "SQL", sql: "select 1",
  catalog: "app", schema: null, createdAt: "now", updatedAt: "now",
};

function actions() {
  return {
    connect: vi.fn(), delete: vi.fn(), deleteSavedSql: vi.fn(), duplicate: vi.fn(), designTable: vi.fn(),
    disconnect: vi.fn(), edit: vi.fn(), newConnection: vi.fn(), newQuery: vi.fn(), openSavedSql: vi.fn(),
    previewTable: vi.fn(), refresh: vi.fn(), refreshSchema: vi.fn(), selectConnection: vi.fn(),
    selectTable: vi.fn(), toggleCatalog: vi.fn(), toggleConnection: vi.fn(), useSql: vi.fn(),
  } satisfies DatabaseSidebarActions;
}

function options() {
  return {
    active: true, workspaceId: "ws", workspaceName: "Workspace",
    catalogNamesByConn: { pg: ["app", "analytics"] }, connections: [pg],
    connectionStates: { pg: { status: "connected" as const } }, executePending: false,
    savedSql: [saved], selectedConnectionId: "pg", selectedTableId: "pg-table",
    sidebarActions: actions(), toolbarConnection: pg, toolbarSession: { status: "connected" as const },
    treeErrors: { "pg::analytics": "unavailable" }, treeLoadingKeys: ["pg::app"], treeSchemaCache: { "pg::app": schema("pg", users) },
    onShellSidebarChange: vi.fn(), onShellStatusBarChange: vi.fn(), statusBarRightAccessory: <span>Accessory</span>,
  };
}

function latestSidebar(initialOptions: ReturnType<typeof options>) {
  return initialOptions.onShellSidebarChange.mock.lastCall![0] as ReactElement<ComponentProps<typeof DatabaseSidebar>>;
}
function latestStatus(initialOptions: ReturnType<typeof options>) {
  return initialOptions.onShellStatusBarChange.mock.lastCall![0] as ReactElement<ComponentProps<typeof DatabaseStatusBar>>;
}

describe("database shell integration", () => {
  it.each([
    { kind: "table", readOnly: false },
    { kind: "table", readOnly: true },
    { kind: "view", readOnly: true },
  ])("forwards Design Table from the sidebar menu for $kind (readOnly=$readOnly)", async ({ kind, readOnly }) => {
    const target = { ...users, kind };
    const initial = {
      ...options(), connections: [{ ...pg, readOnly }], treeSchemaCache: { "pg::app": schema("pg", target) },
    };
    renderHook(useDatabaseShellIntegration, { initialProps: initial });
    render(latestSidebar(initial));
    if (kind === "view") {
      const group = screen.getByRole("button", { name: "Views" }).closest("[role='treeitem']")!;
      fireEvent.click(within(group as HTMLElement).getByRole("button", { name: "Expand" }));
    }
    fireEvent.contextMenu(screen.getByRole("button", { name: "users" }));
    const menu = within(await screen.findByRole("menu"));
    expect(Boolean(menu.queryByRole("menuitem", { name: "Generate INSERT Statement" }))).toBe(kind === "table");
    expect(Boolean(menu.queryByRole("menuitem", { name: "Export Table…" }))).toBe(kind === "table");
    fireEvent.click(menu.getByRole("menuitem", { name: "Design Table" }));
    expect(initial.sidebarActions.designTable).toHaveBeenCalledExactlyOnceWith("pg", target);
    expect(initial.sidebarActions.designTable.mock.lastCall![1]).toBe(target);
  });

  it("injects tree/status data and keeps every handler stable while committing the latest actions", () => {
    const initial = options();
    const { rerender } = renderHook(useDatabaseShellIntegration, { initialProps: initial });
    const sidebar = latestSidebar(initial);
    expect(sidebar.type).toBe(DatabaseSidebar);
    expect(sidebar.props).toMatchObject({
      catalogNamesByConnection: initial.catalogNamesByConn, schemaCache: initial.treeSchemaCache,
      connectionStates: initial.connectionStates, loadingKeys: initial.treeLoadingKeys, loadErrors: initial.treeErrors,
      selectedConnectionId: "pg", selectedTableId: "pg-table", savedSqlByConnection: { pg: [saved] },
    });
    expect(latestStatus(initial).type).toBe(DatabaseStatusBar);
    expect(latestStatus(initial).props).toMatchObject({
      connection: pg, executing: false, session: initial.toolbarSession,
      rightAccessory: initial.statusBarRightAccessory, workspaceName: "Workspace",
    });
    const updatedActions = actions();
    rerender({ ...initial, sidebarActions: updatedActions });
    expect(initial.onShellSidebarChange).toHaveBeenCalledTimes(1);
    expect(initial.onShellStatusBarChange).toHaveBeenCalledTimes(1);
    const handlers = sidebar.props;
    handlers.onConnect(pg);
    handlers.onDeleteConnection(pg);
    handlers.onDeleteSavedSql!(saved);
    handlers.onDuplicateConnection!(pg);
    handlers.onDesignTable!("pg", users);
    handlers.onDisconnect(pg);
    handlers.onEditConnection(pg);
    handlers.onNewConnection();
    handlers.onNewQuery!(pg, "analytics");
    handlers.onOpenSavedSql!(saved);
    handlers.onPreviewTable("pg", users);
    handlers.onRefresh();
    handlers.onRefreshSchema(pg);
    handlers.onSelectConnection(pg);
    handlers.onSelectTable("pg", users);
    handlers.onToggleCatalog("pg", "analytics");
    handlers.onToggleConnection(pg);
    handlers.onUseSql!("pg", "select 1", users);
    for (const action of Object.keys(updatedActions) as Array<keyof DatabaseSidebarActions>) {
      expect(updatedActions[action]).toHaveBeenCalledTimes(1);
      expect(initial.sidebarActions[action]).not.toHaveBeenCalled();
    }
    expect(updatedActions.newQuery).toHaveBeenCalledWith(pg, "analytics");
    expect(updatedActions.useSql).toHaveBeenCalledWith("pg", "select 1", users);
    expect(updatedActions.toggleCatalog).toHaveBeenCalledWith("pg", "analytics");
    rerender({ ...initial, sidebarActions: updatedActions, selectedConnectionId: "mysql" });
    const nextSidebar = latestSidebar(initial);
    for (const key of Object.keys(handlers).filter((key) => key.startsWith("on"))) {
      expect(nextSidebar.props[key as keyof typeof handlers]).toBe(handlers[key as keyof typeof handlers]);
    }
    expect(initial.onShellSidebarChange.mock.calls.slice(-2).map(([node]) => node)).toEqual([null, nextSidebar]);
  });

  it("updates status independently and clears both slots on deactivation, callback replacement and unmount", () => {
    const initial = options();
    const { rerender, unmount } = renderHook(useDatabaseShellIntegration, { initialProps: initial });
    const updated = { ...initial, executePending: true, toolbarConnection: connection("mysql", "mysql") };
    rerender(updated);
    expect(initial.onShellSidebarChange).toHaveBeenCalledTimes(1);
    expect(latestStatus(initial).props).toMatchObject({ connection: updated.toolbarConnection, executing: true });
    const replacement = { ...updated, onShellSidebarChange: vi.fn(), onShellStatusBarChange: vi.fn() };
    rerender(replacement);
    expect(initial.onShellSidebarChange).toHaveBeenLastCalledWith(null);
    expect(initial.onShellStatusBarChange).toHaveBeenLastCalledWith(null);
    expect(latestSidebar(replacement).type).toBe(DatabaseSidebar);
    rerender({ ...replacement, active: false });
    expect(replacement.onShellSidebarChange).toHaveBeenLastCalledWith(null);
    expect(replacement.onShellStatusBarChange).toHaveBeenLastCalledWith(null);
    const count = replacement.onShellSidebarChange.mock.calls.length;
    rerender({ ...replacement, active: false, selectedTableId: "other" });
    expect(replacement.onShellSidebarChange).toHaveBeenCalledTimes(count);
    rerender(replacement);
    expect(latestSidebar(replacement).type).toBe(DatabaseSidebar);
    unmount();
    expect(replacement.onShellSidebarChange).toHaveBeenLastCalledWith(null);
    expect(replacement.onShellStatusBarChange).toHaveBeenLastCalledWith(null);
  });

  it("supports absent shell callbacks and falls back to the workspace id", () => {
    const initial = options();
    const { rerender } = renderHook<void, Parameters<typeof useDatabaseShellIntegration>[0]>(useDatabaseShellIntegration, {
      initialProps: { ...initial, workspaceName: undefined, onShellSidebarChange: undefined, onShellStatusBarChange: undefined },
    });
    expect(initial.onShellSidebarChange).not.toHaveBeenCalled();
    rerender({ ...initial, workspaceName: undefined, onShellSidebarChange: undefined, onShellStatusBarChange: initial.onShellStatusBarChange });
    expect(latestStatus(initial).props.workspaceName).toBe("ws");
  });
});
