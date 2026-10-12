import type {
  DatabaseBrowseInput, DatabaseConnectionInput, DatabaseQueryInput,
  DatabaseRowMutationInput, DatabaseTableStructureInput, SavedSqlInput,
} from "../../types";
import {
  deleteMockDatabaseConnection, listMockDatabaseConnections,
  saveMockDatabaseConnection, testMockDatabaseConnection,
} from "./database-connections";
import {
  browseMockDatabaseTable, executeMockDatabaseQuery,
  getMockDatabaseTableStructure, mutateMockDatabaseRow,
} from "./database-queries";
import { deleteMockSavedSql, listMockSavedSql, saveMockSavedSql } from "./database-saved-sql";
import { getMockDatabaseSchema, listMockDatabaseCatalogs } from "./database-schema";
import { mockState } from "./state";
import { UNHANDLED, type MockResult } from "./types";

type MockArgs = Record<string, unknown> | undefined;

function workspaceId(args: MockArgs) {
  return String(args?.workspaceId ?? mockState.activeWorkspaceId);
}

function connectionId(args: MockArgs) {
  return String(args?.connectionId ?? "");
}

function unsupportedScript() {
  // Browser fixtures cannot guarantee physical-session SQL semantics.
  throw { code: "UNSUPPORTED_OPERATION", message: "SQL script execution requires the desktop database engine." };
}

// Own argument decoding here; each operation owns its fixture/state behavior.
// Map avoids treating inherited object properties as registered commands.
const handlers = new Map<string, (args: MockArgs) => unknown>([
  ["database_connections_list", (args) => listMockDatabaseConnections(workspaceId(args))],
  ["database_connection_save", (args) => saveMockDatabaseConnection(
    args?.input as DatabaseConnectionInput, args?.secret as string | null,
  )],
  ["database_connection_delete", (args) => deleteMockDatabaseConnection(workspaceId(args), connectionId(args))],
  ["database_connection_test", (args) => testMockDatabaseConnection(connectionId(args))],
  ["database_catalogs_list", (args) => listMockDatabaseCatalogs(connectionId(args))],
  ["database_schema_get", (args) => getMockDatabaseSchema(connectionId(args), args?.catalog ? String(args.catalog) : null)],
  ["database_query_history_list", () => []],
  ["database_query_history_record", () => undefined],
  ["database_query_history_clear", () => undefined],
  ["database_saved_sql_list", (args) => listMockSavedSql(workspaceId(args))],
  ["database_saved_sql_save", (args) => saveMockSavedSql(args?.input as SavedSqlInput)],
  ["database_saved_sql_delete", (args) => deleteMockSavedSql(workspaceId(args), String(args?.id ?? ""))],
  ["database_query_execute", (args) => executeMockDatabaseQuery(args?.input as DatabaseQueryInput)],
  ["database_table_browse", (args) => browseMockDatabaseTable(args?.input as DatabaseBrowseInput)],
  ["database_table_structure", (args) => getMockDatabaseTableStructure(args?.input as DatabaseTableStructureInput)],
  ["database_row_mutate", (args) => mutateMockDatabaseRow(args?.input as DatabaseRowMutationInput)],
  ["database_script_execute", unsupportedScript],
  ["database_script_stop", unsupportedScript],
]);

export function handleDatabaseMock<T>(command: string, args?: Record<string, unknown>): MockResult<T> {
  const handler = handlers.get(command);
  // A handled command may return undefined; only a missing handler falls through.
  return handler ? handler(args) as T : UNHANDLED;
}
