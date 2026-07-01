// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import type { DatabaseConnection, DatabaseTable } from "@unfour/command-client";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
});
