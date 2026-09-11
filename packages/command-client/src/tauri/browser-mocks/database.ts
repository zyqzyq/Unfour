import { quoteMySqlIdentifier } from "./helpers";
import { mockState, mockStore } from "./state";
import { UNHANDLED, type MockResult } from "./types";
import type {
  DatabaseBrowseInput,
  DatabaseBrowseResult,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryInput,
  DatabaseQueryResult,
  DatabaseRowMutationInput,
  DatabaseRowMutationResult,
  DatabaseSchema,
  DatabaseTableStructure,
  DatabaseTableStructureInput,
  DatabaseTestResult,
  SavedSql,
  SavedSqlInput,
} from "../../types";

export function handleDatabaseMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "database_script_execute" || command === "database_script_stop") {
    // Browser fixtures cannot guarantee physical-session SQL semantics.
    throw { code: "UNSUPPORTED_OPERATION", message: "SQL script execution requires the desktop database engine." };
  }
  if (command === "database_connections_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockStore.databaseConnections.filter(
      (item) => item.workspaceId === workspaceId,
    ) as T;
  }

  if (command === "database_connection_save") {
    const input = args?.input as DatabaseConnectionInput;
    const now = new Date().toISOString();
    const existingIndex = input.id
      ? mockStore.databaseConnections.findIndex((item) => item.id === input.id)
      : -1;
    const connection: DatabaseConnection = {
      id: input.id || crypto.randomUUID(),
      workspaceId: input.workspaceId,
      name: input.name,
      driver: input.driver,
      host: input.host ?? null,
      port: input.port ?? null,
      database: input.database ?? null,
      username: input.username ?? null,
      sslMode: input.sslMode ?? null,
      sqlitePath: input.sqlitePath ?? null,
      credentialRef: input.credentialRef ?? null,
      readOnly: input.readOnly ?? false,
      createdAt:
        existingIndex >= 0
          ? mockStore.databaseConnections[existingIndex].createdAt
          : now,
      updatedAt: now,
      deletedAt: null,
      revision:
        existingIndex >= 0
          ? mockStore.databaseConnections[existingIndex].revision + 1
          : 1,
      syncStatus: existingIndex >= 0 ? "pending" : "local",
      remoteId: null,
    };
    if (existingIndex >= 0) {
      mockStore.databaseConnections[existingIndex] = connection;
    } else {
      mockStore.databaseConnections = [connection, ...mockStore.databaseConnections];
    }
    return connection as T;
  }

  if (command === "database_connection_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const connectionId = String(args?.connectionId ?? "");
    mockStore.databaseConnections = mockStore.databaseConnections.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === connectionId),
    );
    return mockStore.databaseConnections.filter(
      (item) => item.workspaceId === workspaceId,
    ) as T;
  }

  if (command === "database_connection_test") {
    const connectionId = String(args?.connectionId ?? "");
    const connection = mockStore.databaseConnections.find((item) => item.id === connectionId);
    const driver = connection?.driver ?? "sqlite";
    const isSqlite = driver === "sqlite";
    const isPostgres = driver === "postgres";
    const isMySql = driver === "mysql";
    const ok = isSqlite || isPostgres || isMySql;
    return ({
      ok,
      message: `${isSqlite ? "SQLite" : isPostgres ? "PostgreSQL" : "MySQL"} connection OK`,
      serverVersion: isSqlite
        ? "mock-sqlite-3.x"
        : isPostgres
          ? "mock-postgresql-16.x"
          : "mock-mysql-8.x",
    } satisfies DatabaseTestResult) as T;
  }

  if (command === "database_catalogs_list") {
    const connectionId = String(args?.connectionId ?? "");
    const connection = mockStore.databaseConnections.find((item) => item.id === connectionId);
    if (connection?.driver === "mysql") {
      return ["app", "analytics"] as T;
    }
    if (connection?.driver === "postgres") {
      // Server-level database list; the connection's default plus a second db
      // demonstrate browsing beyond the connected database.
      return [connection.database ?? "postgres", "reporting"] as T;
    }
    return [] as T;
  }

  if (command === "database_schema_get") {
    const connectionId = String(args?.connectionId ?? "");
    const catalogArg = args?.catalog ? String(args.catalog) : null;
    const connection = mockStore.databaseConnections.find((item) => item.id === connectionId);
    const isPostgres = connection?.driver === "postgres";
    const isMySql = connection?.driver === "mysql";
    if (isMySql) {
      return ({
        connectionId,
        tables: [
          {
            catalog: connection.database ?? "app",
            name: "users",
            kind: "table",
            columns: [
              { name: "id", dataType: "bigint unsigned", nullable: false, primaryKey: true },
              { name: "email", dataType: "varchar(255)", nullable: false, primaryKey: false },
              { name: "created_at", dataType: "datetime", nullable: false, primaryKey: false },
            ],
          },
          {
            catalog: "analytics",
            name: "events",
            kind: "table",
            columns: [
              { name: "id", dataType: "bigint unsigned", nullable: false, primaryKey: true },
              { name: "event_name", dataType: "varchar(255)", nullable: false, primaryKey: false },
            ],
          },
        ],
      } satisfies DatabaseSchema) as T;
    }
    if (isPostgres) {
      const pgCatalog = catalogArg ?? connection.database ?? "postgres";
      // A second database (reporting) shows distinct objects, demonstrating
      // catalog-scoped schema browsing on a single connection.
      if (pgCatalog === "reporting") {
        return ({
          connectionId,
          tables: [
            {
              catalog: pgCatalog,
              schema: "metrics",
              name: "daily_revenue",
              kind: "view",
              columns: [
                { name: "day", dataType: "date", nullable: false, primaryKey: false },
                { name: "amount", dataType: "numeric", nullable: true, primaryKey: false },
              ],
            },
          ],
        } satisfies DatabaseSchema) as T;
      }
      return ({
        connectionId,
        tables: [
          {
            catalog: pgCatalog,
            schema: "public",
            name: "users",
            kind: "table",
            columns: [
              { name: "id", dataType: "integer", nullable: false, primaryKey: true },
              {
                name: "email",
                dataType: "character varying",
                nullable: false,
                primaryKey: false,
              },
              {
                name: "created_at",
                dataType: "timestamp with time zone",
                nullable: false,
                primaryKey: false,
              },
            ],
          },
          {
            catalog: pgCatalog,
            schema: "public",
            name: "orders",
            kind: "table",
            columns: [
              { name: "id", dataType: "integer", nullable: false, primaryKey: true },
              { name: "user_id", dataType: "integer", nullable: false, primaryKey: false },
              { name: "total", dataType: "numeric", nullable: true, primaryKey: false },
            ],
          },
        ],
      } satisfies DatabaseSchema) as T;
    }
    return ({
      connectionId,
      tables: [
        {
          name: "api_history",
          kind: "table",
          columns: [
            { name: "id", dataType: "TEXT", nullable: false, primaryKey: true },
            { name: "method", dataType: "TEXT", nullable: false, primaryKey: false },
            { name: "status", dataType: "INTEGER", nullable: true, primaryKey: false },
          ],
        },
        {
          name: "workspaces",
          kind: "table",
          columns: [
            { name: "id", dataType: "TEXT", nullable: false, primaryKey: true },
            { name: "name", dataType: "TEXT", nullable: false, primaryKey: false },
          ],
        },
      ],
    } satisfies DatabaseSchema) as T;
  }

  if (command === "database_query_history_list") {
    return [] as T;
  }

  if (command === "database_query_history_record") {
    return undefined as T;
  }

  if (command === "database_query_history_clear") {
    return undefined as T;
  }

  if (command === "database_saved_sql_list") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    return mockStore.savedSql.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_saved_sql_save") {
    const input = args?.input as SavedSqlInput;
    const workspaceId = input.workspaceId;
    const id = input.id?.trim();
    const name = input.name.trim();
    const sql = input.sql.trim();
    if (!name) throw new Error("saved SQL name cannot be empty");
    if (name.length > 120) throw new Error("saved SQL name must be 120 characters or fewer");
    if (!sql) throw new Error("saved SQL cannot be empty");
    const now = new Date().toISOString();
    const existingIndex = id
      ? mockStore.savedSql.findIndex((item) => item.id === id && item.workspaceId === workspaceId)
      : -1;
    if (id && existingIndex === -1) throw new Error("saved SQL not found");
    const saved: SavedSql = {
      id: id || crypto.randomUUID(),
      workspaceId,
      connectionId: input.connectionId ?? null,
      name,
      sql,
      createdAt: existingIndex >= 0 ? mockStore.savedSql[existingIndex].createdAt : now,
      updatedAt: now,
    };
    if (existingIndex >= 0) {
      mockStore.savedSql = [
        saved,
        ...mockStore.savedSql.filter((_item, index) => index !== existingIndex),
      ];
    } else {
      mockStore.savedSql = [saved, ...mockStore.savedSql];
    }
    return saved as T;
  }

  if (command === "database_saved_sql_delete") {
    const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
    const id = String(args?.id ?? "");
    const initialLength = mockStore.savedSql.length;
    mockStore.savedSql = mockStore.savedSql.filter(
      (item) => !(item.workspaceId === workspaceId && item.id === id),
    );
    if (mockStore.savedSql.length === initialLength) throw new Error("saved SQL not found");
    return mockStore.savedSql.filter((item) => item.workspaceId === workspaceId) as T;
  }

  if (command === "database_query_execute") {
    const input = args?.input as DatabaseQueryInput;
    const isSelect = input.sql.trim().toLowerCase().startsWith("select");
    const keyword = input.sql.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
    const requiresConfirmation = ![
      "select",
      "with",
      "pragma",
      "explain",
      "show",
    ].includes(keyword);
    if (requiresConfirmation && !input.confirmMutation) {
      throw {
        code: "CONFIRMATION_REQUIRED",
        message: "confirmation required: This SQL statement may change data. Confirm to execute it.",
        details: {
          classification: ["insert", "update", "delete", "replace"].includes(keyword)
            ? "mutation"
            : "schema-change",
          requiresConfirmation: true,
          confirmed: false,
        },
      };
    }
    return ({
      columns: isSelect
        ? [
            { name: "id", dataType: "TEXT" },
            { name: "name", dataType: "TEXT" },
            { name: "sync_status", dataType: "TEXT" },
          ]
        : [],
      rows: isSelect
        ? [
            ["mock-workspace", "Default Workspace", "local"],
            ["mock-api", "API Client", "local"],
          ]
        : [],
      affectedRows: isSelect ? 0 : 1,
      durationMs: 7,
      safety: {
        classification: isSelect ? "read" : "mutation",
        requiresConfirmation,
        confirmed: !requiresConfirmation || input.confirmMutation === true,
        message: requiresConfirmation
          ? "This SQL statement may change data. Confirm to execute it."
          : null,
      },
    } satisfies DatabaseQueryResult) as T;
  }

  if (command === "database_table_browse") {
    const input = args?.input as DatabaseBrowseInput;
    const limit = input.limit ?? 100;
    const offset = input.offset ?? 0;
    const mockRows = [
      ["mock-workspace", "Default Workspace", "local"],
      ["mock-api", "API Client", "local"],
      ["mock-db", "Database", "local"],
      ["mock-ssh", "SSH Terminal", "reserved"],
    ];
    const connection = mockStore.databaseConnections.find(
      (item) => item.id === input.connectionId,
    );
    const qualifiedTable =
      connection?.driver === "mysql"
        ? `${quoteMySqlIdentifier(input.catalog ?? input.schema ?? connection.database ?? "app")}.${quoteMySqlIdentifier(input.tableName)}`
        : `"${input.tableName.split('"').join('""')}"`;
    return ({
      tableName: input.tableName,
      sql: `SELECT * FROM ${qualifiedTable} LIMIT ${limit} OFFSET ${offset}`,
      limit,
      offset,
      totalRows: mockRows.length,
      readOnly: true,
      result: {
        columns: [
          { name: "id", dataType: "TEXT" },
          { name: "name", dataType: "TEXT" },
          { name: "sync_status", dataType: "TEXT" },
        ],
        rows: mockRows.slice(offset, offset + limit),
        affectedRows: 0,
        durationMs: 5,
        safety: {
          classification: "read",
          requiresConfirmation: false,
          confirmed: true,
          message: null,
        },
      },
    } satisfies DatabaseBrowseResult) as T;
  }

  if (command === "database_table_structure") {
    const input = args?.input as DatabaseTableStructureInput;
    return ({
      catalog: input.catalog ?? null,
      schema: input.schema ?? null,
      name: input.tableName,
      kind: "table",
      columns: [
        {
          name: "id",
          dataType: "TEXT",
          nullable: false,
          primaryKey: true,
          defaultValue: null,
        },
        {
          name: "name",
          dataType: "TEXT",
          nullable: false,
          primaryKey: false,
          defaultValue: null,
        },
        {
          name: "sync_status",
          dataType: "TEXT",
          nullable: true,
          primaryKey: false,
          defaultValue: "'local'",
        },
      ],
      indexes: [{ name: "PRIMARY", columns: ["id"], unique: true, primary: true }],
      foreignKeys: [],
      ddl: `CREATE TABLE ${input.tableName} (\n  id TEXT PRIMARY KEY,\n  name TEXT NOT NULL,\n  sync_status TEXT DEFAULT 'local'\n);`,
    } satisfies DatabaseTableStructure) as T;
  }

  if (command === "database_row_mutate") {
    const input = args?.input as DatabaseRowMutationInput;
    if (!input.confirmMutation) {
      throw {
        code: "CONFIRMATION_REQUIRED",
        message: "Row changes require explicit confirmation.",
      };
    }
    return ({
      affectedRows: 1,
      sql: `-- mock ${input.operation} on ${input.tableName}`,
    } satisfies DatabaseRowMutationResult) as T;
  }

  return UNHANDLED;
}
