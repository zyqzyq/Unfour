import type {
  DatabaseBrowseInput, DatabaseBrowseResult, DatabaseQueryInput, DatabaseQueryResult,
  DatabaseRowMutationInput, DatabaseRowMutationResult, DatabaseTableStructure, DatabaseTableStructureInput,
} from "../../types";
import { quoteMySqlIdentifier } from "./helpers";
import { mockStore } from "./state";

export function executeMockDatabaseQuery(input: DatabaseQueryInput) {
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
  } satisfies DatabaseQueryResult);
}

export function browseMockDatabaseTable(input: DatabaseBrowseInput) {
  const limit = input.limit ?? 100;
  const offset = input.offset ?? 0;
  const mockRows = [
    ["mock-workspace", "Default Workspace", "local"],
    ["mock-api", "API Client", "local"],
    ["mock-db", "Database", "local"],
    ["mock-ssh", "SSH Terminal", "reserved"],
  ];
  const qualifiedTable = qualifiedBrowseTable(input);
  return ({
    tableName: input.tableName,
    sql: `SELECT * FROM ${qualifiedTable} LIMIT ${limit} OFFSET ${offset}`,
    limit,
    offset,
    totalRows: input.includeTotal === false ? Math.min(mockRows.length, offset + limit + 1) : mockRows.length,
    totalRowsExact: input.includeTotal !== false || offset + limit >= mockRows.length,
    hasMore: offset + limit < mockRows.length,
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
  } satisfies DatabaseBrowseResult);
}

function qualifiedBrowseTable(input: DatabaseBrowseInput) {
  const connection = mockStore.databaseConnections.find(
    (item) => item.id === input.connectionId,
  );
  return connection?.driver === "mysql"
      ? `${quoteMySqlIdentifier(input.catalog ?? input.schema ?? connection.database ?? "app")}.${quoteMySqlIdentifier(input.tableName)}`
      : `"${input.tableName.split('"').join('""')}"`;
}

export function getMockDatabaseTableStructure(input: DatabaseTableStructureInput) {
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
    capabilities: { indexes: true, foreignKeys: true, ddl: true },
    ddl: `CREATE TABLE ${input.tableName} (\n  id TEXT PRIMARY KEY,\n  name TEXT NOT NULL,\n  sync_status TEXT DEFAULT 'local'\n);`,
  } satisfies DatabaseTableStructure);
}

export function mutateMockDatabaseRow(input: DatabaseRowMutationInput) {
  if (!input.confirmMutation) {
    throw {
      code: "CONFIRMATION_REQUIRED",
      message: "Row changes require explicit confirmation.",
    };
  }
  return ({
    affectedRows: 1,
    sql: `-- mock ${input.operation} on ${input.tableName}`,
  } satisfies DatabaseRowMutationResult);
}
