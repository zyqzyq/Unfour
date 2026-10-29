// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { DatabaseSidebar } from "./DatabaseSidebar";
import { connection, schema, table } from "../hooks/database-controller.test-fixtures";

afterEach(cleanup);
const pg = { ...connection("pg", "postgres"), name: "Production" };
const users = table("app", "users", "public");
const saved = { id: "sql", workspaceId: "ws", connectionId: "pg", catalog: "reporting", name: "Daily report", sql: "select secret_text", createdAt: "now", updatedAt: "now" };
function props() {
  return {
    workspaceId: "ws", connections: [pg, { ...connection("other", "sqlite"), name: "Local" }],
    connectionStates: { pg: { status: "connected" as const } },
    catalogNamesByConnection: { pg: ["app", "reporting", "unloaded"] },
    schemaCache: { "pg::app": schema("pg", users, table("app", "orders", "private")) },
    savedSqlByConnection: { pg: [saved] }, selectedConnectionId: "pg", selectedTableId: "pg:table:app:public:users",
    onConnect: vi.fn(), onDisconnect: vi.fn(), onDeleteConnection: vi.fn(), onEditConnection: vi.fn(),
    onNewConnection: vi.fn(), onNewQuery: vi.fn(), onOpenSavedSql: vi.fn(), onDeleteSavedSql: vi.fn(),
    onPreviewTable: vi.fn(), onRefresh: vi.fn(), onRefreshSchema: vi.fn(), onSelectConnection: vi.fn(),
    onSelectTable: vi.fn(), onToggleCatalog: vi.fn(), onToggleConnection: vi.fn(), onUseSql: vi.fn(),
  };
}
function search(value: string) {
  fireEvent.change(screen.getByRole("textbox", { name: "Search database resources" }), { target: { value } });
}
function toggle(name: string, action: "Expand" | "Collapse") {
  const row = screen.getByRole("button", { name, exact: true }).closest("[role=treeitem]") as HTMLElement;
  fireEvent.click(within(row).getAllByRole("button", { name: action, exact: true })[0]);
}

describe("Database sidebar local search", () => {
  it("reveals matching descendants with their full path without loading, selecting or connecting", () => {
    const p = props();
    render(<DatabaseSidebar {...p} />);
    toggle("Production", "Collapse");
    search("  UsErS  ");
    for (const name of ["Production", "app", "public", "Tables", "users"]) {
      expect(screen.getByRole("button", { name, exact: true })).toBeInTheDocument();
    }
    for (const name of ["Local", "reporting", "orders", "private"]) {
      expect(screen.queryByRole("button", { name, exact: true })).not.toBeInTheDocument();
    }
    expect(screen.getByRole("button", { name: "users" }).closest("[role=treeitem]")).toHaveAttribute("aria-selected", "true");
    search("report");
    expect(screen.getByRole("button", { name: "reporting" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Daily report" })).toBeInTheDocument();
    toggle("reporting", "Collapse");
    toggle("reporting", "Expand");
    search("unloaded");
    expect(screen.getByRole("button", { name: "unloaded" })).toBeInTheDocument();
    for (const fn of [p.onConnect, p.onDisconnect, p.onToggleCatalog, p.onToggleConnection, p.onSelectTable, p.onSelectConnection]) expect(fn).not.toHaveBeenCalled();
    search("secret_text");
    expect(screen.getByText("No matching database resources")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Production" }).closest("[role=treeitem]")).toHaveAttribute("aria-expanded", "false");
    toggle("Production", "Expand");
    expect(p.onToggleConnection).toHaveBeenCalledExactlyOnceWith(pg);
    expect(screen.getByRole("button", { name: "users" }).closest("[role=treeitem]")).toHaveAttribute("aria-selected", "true");
  });

  it.each(["Production", "app", "public", "Saved Queries"])("retains loaded descendants for a matching parent: %s", (query) => {
    render(<DatabaseSidebar {...props()} />);
    search(query);
    expect(screen.getByRole("button", { name: query === "Saved Queries" ? "Daily report" : "users" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Local" })).not.toBeInTheDocument();
  });

  it("preserves table selection, preview and saved-query actions while filtering", async () => {
    const p = props();
    render(<DatabaseSidebar {...p} />);
    search("users");
    fireEvent.click(screen.getByRole("button", { name: "users" }));
    expect(p.onSelectTable).toHaveBeenCalledExactlyOnceWith("pg", users);
    expect(p.onPreviewTable).not.toHaveBeenCalled();
    fireEvent.doubleClick(screen.getByRole("button", { name: "users" }));
    expect(p.onPreviewTable).toHaveBeenCalledExactlyOnceWith("pg", users);
    search("Daily");
    fireEvent.doubleClick(screen.getByRole("button", { name: "Daily report" }));
    expect(p.onOpenSavedSql).toHaveBeenCalledExactlyOnceWith(saved);
    fireEvent.contextMenu(screen.getByRole("button", { name: "Daily report" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    expect(p.onDeleteSavedSql).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(p.onDeleteSavedSql).not.toHaveBeenCalled();
  });

  it("resets search on Workspace changes and excludes old workspace records", () => {
    const p = props();
    const view = render(<DatabaseSidebar {...p} />);
    search("users");
    view.rerender(<DatabaseSidebar {...p} workspaceId="ws-2" connections={[...p.connections, { ...connection("new", "sqlite"), workspaceId: "ws-2", name: "New workspace" }]} />);
    expect(screen.getByRole("textbox")).toHaveValue("");
    expect(screen.getByRole("button", { name: "New workspace" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Production" })).not.toBeInTheDocument();
    search("users");
    expect(screen.getByText("No matching database resources")).toBeInTheDocument();
    view.rerender(<DatabaseSidebar {...p} />);
    expect(screen.getByRole("textbox")).toHaveValue("");
  });

  it("localizes search, clear and no results in Chinese", () => {
    render(<I18nProvider initialLocale="zh-CN"><DatabaseSidebar {...props()} /></I18nProvider>);
    const input = screen.getByRole("textbox", { name: "搜索数据库资源" });
    fireEvent.change(input, { target: { value: "不存在" } });
    expect(screen.getByText("没有匹配的数据库资源")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清空搜索" }));
    expect(input).toHaveValue("");
  });
});
