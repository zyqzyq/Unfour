import type { DatabaseSchema } from "../../types";
import { mockStore } from "./state";

export function listMockDatabaseCatalogs(connectionId: string) {
  const connection = mockStore.databaseConnections.find((item) => item.id === connectionId);
  if (connection?.driver === "mysql") {
    return ["app", "analytics"];
  }
  if (connection?.driver === "postgres") {
    // Server-level database list; the connection's default plus a second db
    // demonstrate browsing beyond the connected database.
    return [connection.database ?? "postgres", "reporting"];
  }
  return [];
}

export function getMockDatabaseSchema(connectionId: string, catalogArg: string | null) {
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
    } satisfies DatabaseSchema);
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
      } satisfies DatabaseSchema);
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
    } satisfies DatabaseSchema);
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
  } satisfies DatabaseSchema);
}
