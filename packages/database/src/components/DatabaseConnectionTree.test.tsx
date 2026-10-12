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

  it("offers Copy DDL and Export Table in the table context menu", async () => {
    renderTree({ connectionStates: connectedState, schemaCache: {
      "conn-1::": { connectionId: "conn-1", tables: [usersTable] },
    } });
    fireEvent.contextMenu(await screen.findByRole("button", { name: "users" }));
    expect(await screen.findByRole("menuitem", { name: "Copy DDL" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "Export Table…" }));
    expect(await screen.findByRole("dialog", { name: "Export Table" })).toBeInTheDocument();
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


it("places saved queries under their database and starts queries with that catalog", async () => {
  const server = { ...sqliteConnection, driver: "postgres", database: "analytics", name: "Server" };
  const onNewQuery = vi.fn();
  const onOpenSavedSql = vi.fn();
  const saved = { id: "saved", connectionId: "conn-1", workspaceId: "ws-1", catalog: "analytics", name: "Analytics SQL", sql: "select 1", createdAt: "now", updatedAt: "now" };
  renderTree({ connections: [server], connectionStates: connectedState,
    catalogNamesByConnection: { "conn-1": ["app", "analytics"] },
    schemaCache: { "conn-1::analytics": { connectionId: "conn-1", tables: [] } },
    onNewQuery, onOpenSavedSql,
    savedSqlByConnection: { "conn-1": [saved, { ...saved, id: "legacy", name: "Unresolved legacy", catalog: null }] },
  });
  const database = (await screen.findByRole("button", { name: "analytics" })).closest("[role='treeitem']")!;
  expect(screen.getByText("Saved Queries").closest("[role=treeitem]")).toHaveAttribute("data-tree-id", "conn-1:catalog:analytics:saved-sql");
  fireEvent.click(within(database as HTMLElement).getByRole("button", { name: "Collapse" }));
  expect(screen.queryByText("Saved Queries")).not.toBeInTheDocument();
  fireEvent.click(within(database as HTMLElement).getByRole("button", { name: "Expand" }));
  await expandSavedQueries();
  fireEvent.doubleClick(await screen.findByRole("button", { name: "Analytics SQL" }));
  expect(onOpenSavedSql).toHaveBeenCalledWith(saved);
  expect(screen.queryByText("Unresolved legacy")).not.toBeInTheDocument();
  fireEvent.contextMenu(screen.getByRole("button", { name: "analytics" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "New Query" }));
  expect(onNewQuery).toHaveBeenCalledWith(server, "analytics");
});

describe("DatabaseConnectionTree lazy catalog interactions", () => {
  const server = { ...sqliteConnection, driver: "postgres", database: "app" };

  it("routes expansion without selection and preserves a manually collapsed catalog after refresh", () => {
    const onToggleCatalog = vi.fn();
    const onSelectConnection = vi.fn();
    const props = {
      connections: [server], connectionStates: connectedState,
      catalogNamesByConnection: { "conn-1": ["app", "reporting"] },
      onToggleCatalog, onSelectConnection, selectedConnectionId: "conn-1",
    };
    const view = render(<DatabaseConnectionTree {...props} />);
    const row = screen.getByRole("button", { name: "reporting" }).closest("[role=treeitem]") as HTMLElement;
    fireEvent.click(within(row).getByRole("button", { name: "Expand" }));
    expect(onToggleCatalog).toHaveBeenCalledExactlyOnceWith("conn-1", "reporting");
    expect(onSelectConnection).not.toHaveBeenCalled();
    expect(screen.getByText("Expand to load")).toBeInTheDocument();

    view.rerender(<DatabaseConnectionTree {...props} loadingKeys={["conn-1::reporting"]} />);
    expect(screen.getByText("Loading…")).toBeInTheDocument();
    view.rerender(<DatabaseConnectionTree {...props} loadErrors={{ "conn-1::reporting": "Fixture unavailable" }} />);
    expect(screen.getByText("Fixture unavailable")).toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: "Collapse" }));
    expect(onToggleCatalog).toHaveBeenCalledTimes(1);

    view.rerender(<DatabaseConnectionTree {...props} schemaCache={{
      "conn-1::reporting": { connectionId: "conn-1", tables: [{ ...usersTable, catalog: "reporting" }] },
    }} />);
    expect(row).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: "users" })).not.toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: "Expand" }));
    expect(screen.getByRole("button", { name: "users" })).toBeInTheDocument();
  });

  it("preserves schema grouping, stable table IDs, and select versus activate semantics", () => {
    const table = { ...usersTable, catalog: "app", schema: "public" };
    const viewTable = { ...table, name: "active_users", kind: "MATERIALIZED VIEW" };
    const onSelectTable = vi.fn();
    const onPreviewTable = vi.fn();
    renderTree({ connections: [server], connectionStates: connectedState,
      catalogNamesByConnection: { "conn-1": ["app"] },
      schemaCache: { "conn-1::app": { connectionId: "conn-1", tables: [table, viewTable] } },
      onSelectTable, onPreviewTable,
    });
    expect(screen.getByRole("button", { name: "public" })).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "users" });
    expect(button.closest("[role=treeitem]")).toHaveAttribute("data-tree-id", "conn-1:table:app:public:users");
    fireEvent.click(button);
    expect(onSelectTable).toHaveBeenCalledExactlyOnceWith("conn-1", table);
    expect(onPreviewTable).not.toHaveBeenCalled();
    fireEvent.doubleClick(button);
    expect(onPreviewTable).toHaveBeenCalledExactlyOnceWith("conn-1", table);
    expect(screen.queryByRole("button", { name: "active_users" })).not.toBeInTheDocument();
    const views = screen.getByRole("button", { name: "Views" }).closest("[role=treeitem]") as HTMLElement;
    fireEvent.click(within(views).getByRole("button", { name: "Expand" }));
    expect(screen.getByRole("button", { name: "active_users" })).toBeInTheDocument();
  });
});
