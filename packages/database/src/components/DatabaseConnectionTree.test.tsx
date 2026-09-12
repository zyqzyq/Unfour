// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import type { DatabaseConnection, DatabaseTable } from "@unfour/command-client";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DatabaseConnectionTree } from "./DatabaseConnectionTree";

afterEach(cleanup);

const sqliteConnection: DatabaseConnection = {
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

const usersTable: DatabaseTable = {
  catalog: null,
  schema: null,
  name: "users",
  kind: "table",
  columns: [
    {
      name: "id",
      dataType: "integer",
      nullable: false,
      primaryKey: true,
    },
  ],
};

async function expandSavedQueries() {
  const group = (await screen.findByText("Saved Queries")).closest("[role='treeitem']");
  expect(group).toBeTruthy();
  const expand = within(group as HTMLElement).queryByRole("button", { name: "Expand" });
  if (expand) {
    fireEvent.click(expand);
  }
}

const connectedState = {
  "conn-1": {
    message: "Connected",
    serverVersion: null,
    status: "connected",
    updatedAt: "2026-01-01T00:00:00.000Z",
  },
} as const;

function renderTree(
  props: Partial<Parameters<typeof DatabaseConnectionTree>[0]> = {},
) {
  return render(
    <DatabaseConnectionTree
      connections={[sqliteConnection]}
      onSelectConnection={vi.fn()}
      selectedConnectionId="conn-1"
      {...props}
    />,
  );
}

describe("DatabaseConnectionTree", () => {
  it("does not expand a selected connection before it is connected", () => {
    renderTree();

    const connectionRow = screen
      .getByRole("button", { name: "Local SQLite" })
      .closest("[role='treeitem']");

    expect(connectionRow).not.toHaveAttribute("aria-expanded");
    expect(screen.queryByText("Connect to browse databases")).not.toBeInTheDocument();
  });

  it("keeps the selected connection collapsed while it is still connecting", () => {
    renderTree({
      connectionStates: {
        "conn-1": {
          message: "Connecting",
          serverVersion: null,
          status: "connecting",
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
      },
    });

    const connectionRow = screen
      .getByRole("button", { name: "Local SQLite" })
      .closest("[role='treeitem']");

    expect(connectionRow).not.toHaveAttribute("aria-expanded");
    expect(screen.queryByText("Expand to load")).not.toBeInTheDocument();
  });

  it("shows the schema loading placeholder after a connection succeeds", () => {
    renderTree({
      connectionStates: {
        "conn-1": {
          message: "Connected",
          serverVersion: null,
          status: "connected",
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
      },
    });

    expect(screen.getByText("Expand to load")).toBeInTheDocument();
  });

  it("opens table preview from a table double-click", async () => {
    const onPreviewTable = vi.fn();
    renderTree({
      connectionStates: connectedState,
      onPreviewTable,
      schemaCache: {
        "conn-1::": {
          connectionId: "conn-1",
          tables: [usersTable],
        },
      },
    });

    fireEvent.doubleClick(await screen.findByRole("button", { name: "users" }));

    expect(onPreviewTable).toHaveBeenCalledWith("conn-1", usersTable);
  });

  it("passes connection context when New Query is selected from the connection context menu", async () => {
    const onNewQuery = vi.fn();
    renderTree({
      connectionStates: connectedState,
      onNewQuery,
    });

    fireEvent.contextMenu(screen.getByText("Local SQLite"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "New Query" }));

    expect(onNewQuery).toHaveBeenCalledWith(sqliteConnection);
  });

  it("opens saved SQL from the sidebar tree without replacing the current editor", async () => {
    const onOpenSavedSql = vi.fn();
    const savedSql = {
      connectionId: "conn-1",
      createdAt: "2026-01-01T00:00:00.000Z",
      id: "sql-1",
      name: "List users",
      sql: "SELECT * FROM users;",
      updatedAt: "2026-01-01T00:00:00.000Z",
      workspaceId: "ws-1",
    };
    renderTree({
      connectionStates: connectedState,
      onOpenSavedSql,
      savedSqlByConnection: { "conn-1": [savedSql] },
      schemaCache: {
        "conn-1::": {
          connectionId: "conn-1",
          tables: [usersTable],
        },
      },
    });

    await expandSavedQueries();
    fireEvent.doubleClick(await screen.findByRole("button", { name: "List users" }));
    expect(onOpenSavedSql).toHaveBeenCalledWith(savedSql);
  });

  it("asks before deleting saved SQL from the sidebar and keeps it when cancelled", async () => {
    const onDeleteSavedSql = vi.fn();
    const savedSql = {
      connectionId: "conn-1",
      createdAt: "2026-01-01T00:00:00.000Z",
      id: "sql-1",
      name: "List users",
      sql: "SELECT * FROM users;",
      updatedAt: "2026-01-01T00:00:00.000Z",
      workspaceId: "ws-1",
    };
    renderTree({
      connectionStates: connectedState,
      onDeleteSavedSql,
      onOpenSavedSql: vi.fn(),
      savedSqlByConnection: { "conn-1": [savedSql] },
      schemaCache: {
        "conn-1::": {
          connectionId: "conn-1",
          tables: [usersTable],
        },
      },
    });

    await expandSavedQueries();
    fireEvent.contextMenu(await screen.findByText("List users"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    expect(onDeleteSavedSql).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toHaveTextContent('Delete saved query "List users"?');

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onDeleteSavedSql).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("deletes saved SQL from the sidebar only after confirmation", async () => {
    const onDeleteSavedSql = vi.fn();
    const savedSql = {
      connectionId: "conn-1",
      createdAt: "2026-01-01T00:00:00.000Z",
      id: "sql-1",
      name: "List users",
      sql: "SELECT * FROM users;",
      updatedAt: "2026-01-01T00:00:00.000Z",
      workspaceId: "ws-1",
    };
    renderTree({
      connectionStates: connectedState,
      onDeleteSavedSql,
      onOpenSavedSql: vi.fn(),
      savedSqlByConnection: { "conn-1": [savedSql] },
      schemaCache: {
        "conn-1::": {
          connectionId: "conn-1",
          tables: [usersTable],
        },
      },
    });

    await expandSavedQueries();
    fireEvent.contextMenu(await screen.findByText("List users"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(onDeleteSavedSql).toHaveBeenCalledWith(savedSql);
  });
});
