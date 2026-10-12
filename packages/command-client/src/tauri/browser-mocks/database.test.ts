import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DatabaseConnection, DatabaseConnectionInput, DatabaseSchema, SavedSql } from "../../types";
import { handleDatabaseMock } from "./database";
import { mockInvoke } from "./index";
import { mockState, mockStore } from "./state";
import { UNHANDLED } from "./types";

const connectionInput: DatabaseConnectionInput = {
  workspaceId: "database-test", name: "Server", driver: "postgres", database: "app",
};

beforeEach(() => {
  mockStore.databaseConnections = [];
  mockStore.savedSql = [];
  mockStore.credentials = {};
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
});

afterEach(() => {
  mockStore.databaseConnections = [];
  mockStore.savedSql = [];
  mockStore.credentials = {};
  vi.useRealTimers();
});

describe("database browser command routing", () => {
  it("distinguishes unhandled commands from handled void results", async () => {
    expect(handleDatabaseMock("unknown")).toBe(UNHANDLED);
    expect(handleDatabaseMock("toString")).toBe(UNHANDLED);
    await expect(mockInvoke("database_query_history_list")).resolves.toEqual([]);
    await expect(mockInvoke("database_query_history_record")).resolves.toBeUndefined();
    await expect(mockInvoke("database_query_history_clear")).resolves.toBeUndefined();
    await expect(mockInvoke("database_unknown")).rejects.toThrow("Mock command is not implemented");
  });

  it.each(["database_script_execute", "database_script_stop"])("keeps %s unsupported", async (command) => {
    await expect(mockInvoke(command)).rejects.toEqual({
      code: "UNSUPPORTED_OPERATION",
      message: "SQL script execution requires the desktop database engine.",
    });
  });
});

describe("database browser connection records", () => {
  it("preserves defaults, revision, creation time, and connection order on update", async () => {
    const first = await mockInvoke<DatabaseConnection>("database_connection_save", { input: connectionInput });
    expect(first).toMatchObject({
      host: null, port: null, username: null, sslMode: null, sqlitePath: null,
      credentialRef: null, readOnly: false, revision: 1, syncStatus: "local", remoteId: null, deletedAt: null,
    });
    const second = await mockInvoke<DatabaseConnection>("database_connection_save", { input: connectionInput });
    vi.setSystemTime(new Date("2026-01-02T00:00:00Z"));
    const updated = await mockInvoke<DatabaseConnection>("database_connection_save", {
      input: { ...connectionInput, id: first.id, name: "Renamed", readOnly: true },
    });
    expect(updated).toMatchObject({ id: first.id, name: "Renamed", readOnly: true,
      createdAt: first.createdAt, updatedAt: "2026-01-02T00:00:00.000Z", revision: 2, syncStatus: "pending" });
    await expect(mockInvoke("database_connections_list", { workspaceId: connectionInput.workspaceId }))
      .resolves.toEqual([second, updated]);
    await expect(mockInvoke("database_connection_delete", { workspaceId: "other", connectionId: first.id }))
      .resolves.toEqual([]);
    expect(mockStore.databaseConnections).toHaveLength(2);
    await expect(mockInvoke("database_connection_delete", { workspaceId: connectionInput.workspaceId, connectionId: first.id }))
      .resolves.toEqual([second]);
  });

  it("keeps browser credential references and SQLite secret omission unchanged", async () => {
    const server = await mockInvoke<DatabaseConnection>("database_connection_save", {
      input: connectionInput, secret: "fixture-password",
    });
    expect(server.credentialRef).toMatch(/^unfour:database-test:database-password:/);
    expect(mockStore.credentials[server.credentialRef!]).toBe("fixture-password");
    const sqlite = await mockInvoke<DatabaseConnection>("database_connection_save", {
      input: { ...connectionInput, driver: "sqlite", credentialRef: "ignored" }, secret: "ignored",
    });
    expect(sqlite.credentialRef).toBeNull();
    expect(Object.keys(mockStore.credentials)).toHaveLength(1);
  });

  it("defaults list and delete scope to the active workspace", async () => {
    const connection = await mockInvoke<DatabaseConnection>("database_connection_save", {
      input: { ...connectionInput, workspaceId: mockState.activeWorkspaceId },
    });
    await expect(mockInvoke("database_connections_list")).resolves.toEqual([connection]);
    await expect(mockInvoke("database_connection_delete", { connectionId: connection.id })).resolves.toEqual([]);
  });
});

describe("database browser schema fixtures", () => {
  it.each([
    ["sqlite", "mock-sqlite-3.x", []],
    ["postgres", "mock-postgresql-16.x", ["app", "reporting"]],
    ["mysql", "mock-mysql-8.x", ["app", "analytics"]],
  ] as const)("retains %s catalogs and version", async (driver, serverVersion, catalogs) => {
    const connection = await mockInvoke<DatabaseConnection>("database_connection_save", {
      input: { ...connectionInput, driver },
    });
    await expect(mockInvoke("database_connection_test", { connectionId: connection.id }))
      .resolves.toMatchObject({ ok: true, serverVersion });
    await expect(mockInvoke("database_catalogs_list", { connectionId: connection.id })).resolves.toEqual(catalogs);
    const schema = await mockInvoke<DatabaseSchema>("database_schema_get", { connectionId: connection.id });
    expect(schema.connectionId).toBe(connection.id);
    expect(schema.tables.map((table) => table.name)).toEqual(driver === "sqlite"
      ? ["api_history", "workspaces"] : driver === "mysql" ? ["users", "events"] : ["users", "orders"]);
  });

  it("keeps PostgreSQL reporting separate and returns fresh schema objects", async () => {
    const connection = await mockInvoke<DatabaseConnection>("database_connection_save", { input: connectionInput });
    const args = { connectionId: connection.id, catalog: "reporting" };
    const schema = await mockInvoke<DatabaseSchema>("database_schema_get", args);
    expect(schema.tables).toMatchObject([{ catalog: "reporting", schema: "metrics", name: "daily_revenue", kind: "view" }]);
    schema.tables[0].columns.length = 0;
    const fresh = await mockInvoke<DatabaseSchema>("database_schema_get", args);
    expect(fresh.tables[0].columns).toHaveLength(2);
  });
});

describe("database browser saved SQL", () => {
  it.each([
    ["", "select 1", "saved SQL name cannot be empty"],
    ["x".repeat(121), "select 1", "saved SQL name must be 120 characters or fewer"],
    ["Query", "  ", "saved SQL cannot be empty"],
  ])("rejects invalid name/SQL before changing the store (%s)", async (name, sql, message) => {
    await expect(mockInvoke("database_saved_sql_save", { input: { workspaceId: "w", name, sql } })).rejects.toThrow(message);
    expect(mockStore.savedSql).toEqual([]);
  });

  it("moves an updated snippet first while preserving creation time and trimmed context", async () => {
    const input = { workspaceId: "w", name: "Query", sql: "select 1", catalog: " app ", schema: " public " };
    const first = await mockInvoke<SavedSql>("database_saved_sql_save", { input });
    const second = await mockInvoke<SavedSql>("database_saved_sql_save", { input });
    vi.setSystemTime(new Date("2026-01-02T00:00:00Z"));
    const updated = await mockInvoke<SavedSql>("database_saved_sql_save", { input: { ...input, id: ` ${first.id} ` } });
    expect(updated).toMatchObject({ id: first.id, catalog: "app", schema: "public", createdAt: first.createdAt });
    await expect(mockInvoke("database_saved_sql_list", { workspaceId: "w" })).resolves.toEqual([updated, second]);
  });
});

describe("database browser execution fixtures", () => {
  it.each(["UPDATE users SET name = 'x'", "CREATE TABLE users (id int)"])("preserves confirmation for %s", async (sql) => {
    const input = { workspaceId: "w", connectionId: "c", sql };
    await expect(mockInvoke("database_query_execute", { input })).rejects.toMatchObject({
      code: "CONFIRMATION_REQUIRED", details: { requiresConfirmation: true, confirmed: false },
    });
    await expect(mockInvoke("database_query_execute", { input: { ...input, confirmMutation: true } }))
      .resolves.toMatchObject({ affectedRows: 1, safety: { requiresConfirmation: true, confirmed: true } });
  });

  it("keeps row confirmation and table structure metadata", async () => {
    const input = { workspaceId: "w", connectionId: "c", tableName: "users", operation: "delete", catalog: "app", schema: "public" };
    await expect(mockInvoke("database_row_mutate", { input })).rejects.toMatchObject({ code: "CONFIRMATION_REQUIRED" });
    await expect(mockInvoke("database_row_mutate", { input: { ...input, confirmMutation: true } }))
      .resolves.toEqual({ affectedRows: 1, sql: "-- mock delete on users" });
    await expect(mockInvoke("database_table_structure", { input })).resolves.toMatchObject({
      name: "users", catalog: "app", schema: "public", capabilities: { indexes: true, foreignKeys: true, ddl: true },
      indexes: [{ name: "PRIMARY", columns: ["id"], unique: true, primary: true }], foreignKeys: [],
    });
  });

  it.each([[0, 3, false, true], [3, 4, true, false]] as const)(
    "preserves estimated totals and quoted MySQL identifiers at offset %s", async (offset, totalRows, totalRowsExact, hasMore) => {
      const connection = await mockInvoke<DatabaseConnection>("database_connection_save", {
        input: { ...connectionInput, driver: "mysql" },
      });
      await expect(mockInvoke("database_table_browse", { input: {
        connectionId: connection.id, tableName: "odd`table", catalog: "odd`catalog", limit: 2, offset, includeTotal: false,
      } })).resolves.toMatchObject({
        sql: `SELECT * FROM \`odd\`\`catalog\`.\`odd\`\`table\` LIMIT 2 OFFSET ${offset}`,
        totalRows, totalRowsExact, hasMore, readOnly: true,
      });
    },
  );
});
