// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DatabaseConnection, DatabaseTable } from "@unfour/command-client";
import { DatabaseWorkspace } from "./DatabaseWorkspace";
import type { DatabaseWorkspaceTab } from "../model/types";

afterEach(cleanup);

const connection: DatabaseConnection = {
  id: "conn-1",
  workspaceId: "ws-1",
  name: "Local SQLite",
  driver: "sqlite",
  host: null,
  port: null,
  database: null,
  username: null,
  sslMode: null,
  sqlitePath: "D:\\data\\app.sqlite",
  credentialRef: null,
  readOnly: false,
  createdAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
  deletedAt: null,
  revision: 1,
  syncStatus: "local",
  remoteId: null,
};

const table: DatabaseTable = {
  catalog: null,
  schema: null,
  name: "users",
  kind: "table",
  columns: [],
};

const tabs: DatabaseWorkspaceTab[] = [
  {
    activeResultIndex: 0,
    catalog: null,
    connectionId: "conn-1",
    error: null,
    id: "query-1",
    kind: "query",
    pendingConfirmation: false,
    result: null,
    results: [],
    resultTab: "results",
    schema: null,
    sql: "",
    title: "Query 1",
  },
  {
    connectionId: "conn-1",
    error: null,
    id: "table-1",
    kind: "table",
    pendingChanges: [],
    queryResult: null,
    segment: "data",
    structureTab: "ddl",
    table,
    tableQuery: { filter: "", orderBy: null, orderDescending: false },
    tableView: null,
    title: "users",
  },
];

function renderWorkspace(
  props: Partial<Parameters<typeof DatabaseWorkspace>[0]> = {},
) {
  const defaultProps: Parameters<typeof DatabaseWorkspace>[0] = {
    activeTab: tabs[1],
    activeTabId: "table-1",
    catalogOptions: [],
    connections: [connection],
    executePending: false,
    history: [],
    onChangeQueryContext: vi.fn(),
    onClearHistory: vi.fn(),
    onClearSql: vi.fn(),
    onCloseTab: vi.fn(),
    onPreviewSelectedTable: vi.fn(),
    onRefreshSchema: vi.fn(),
    onReorderTabs: vi.fn(),
    onRun: vi.fn(),
    onSelectConnection: vi.fn(),
    onSelectHistory: vi.fn(),
    onSelectResultSet: vi.fn(),
    onSelectResultTab: vi.fn(),
    onSelectStructureTab: vi.fn(),
    onSelectTab: vi.fn(),
    onSelectTableSegment: vi.fn(),
    onShowHistory: vi.fn(),
    onSqlChange: vi.fn(),
    onStop: vi.fn(),
    onTableFilter: vi.fn(),
    onTablePageChange: vi.fn(),
    onTableSort: vi.fn(),
    queryCatalog: null,
    querySchema: null,
    schemaOptions: [],
    schemaError: null,
    tabs,
    workspaceId: "ws-1",
  };

  return render(<DatabaseWorkspace {...defaultProps} {...props} />);
}

describe("DatabaseWorkspace", () => {
  it("renders dynamic tabs and forwards select, close, and reorder callbacks", () => {
    const onCloseTab = vi.fn();
    const onReorderTabs = vi.fn();
    const onSelectTab = vi.fn();
    renderWorkspace({ onCloseTab, onReorderTabs, onSelectTab });

    const queryTab = screen.getByRole("tab", { name: "Query 1" });
    const tableTab = screen.getByRole("tab", { name: /users/ });

    fireEvent.click(queryTab);
    fireEvent.click(screen.getByRole("button", { name: "Close users" }));

    const dataTransfer = { effectAllowed: "", dropEffect: "", setData: vi.fn() };
    fireEvent.dragStart(tableTab.closest("[draggable='true']")!, { dataTransfer });
    fireEvent.drop(queryTab.closest("[draggable='true']")!, { dataTransfer });

    expect(onSelectTab).toHaveBeenCalledWith("query-1");
    expect(onCloseTab).toHaveBeenCalledWith("table-1");
    expect(onReorderTabs).toHaveBeenCalledWith(1, 0);
  });

  it("asks before closing a table with pending row changes", async () => {
    const onCloseTab = vi.fn();
    const dirtyTabs: DatabaseWorkspaceTab[] = tabs.map((tab) =>
      tab.kind === "table"
        ? {
            ...tab,
            pendingChanges: [
              {
                id: "update:1",
                operation: "update",
                originalValues: [],
                primaryKey: [{ column: "id", mode: "value", value: "1" }],
                rowKey: "1",
                values: [{ column: "name", mode: "value", value: "Ada" }],
              },
            ],
          }
        : tab,
    );
    renderWorkspace({ activeTab: dirtyTabs[1], onCloseTab, tabs: dirtyTabs });

    fireEvent.click(screen.getByRole("button", { name: "Close users" }));
    expect(onCloseTab).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(onCloseTab).toHaveBeenCalledWith("table-1"));
  });
});
