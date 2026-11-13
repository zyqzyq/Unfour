import type {
  DatabaseBrowseInput, DatabaseConnectionInput, DatabaseQueryInput,
  DatabaseRowMutationInput, DatabaseTableStructureInput, SavedSqlInput,
} from "../../types";
import {
  deleteMockDatabaseConnection, listMockDatabaseConnections,
  resolveMockDatabaseRoute, saveMockDatabaseConnection, testMockDatabaseConnection,
} from "./database-connections";
import {
  browseMockDatabaseTable, executeMockDatabaseQuery,
  getMockDatabaseTableStructure, mutateMockDatabaseRow,
} from "./database-queries";
import { deleteMockSavedSql, listMockSavedSql, saveMockSavedSql } from "./database-saved-sql";
import { getMockDatabaseSchema, listMockDatabaseCatalogs } from "./database-schema";
import { mockState, mockStore } from "./state";
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
    args?.input as DatabaseConnectionInput, args?.secret as string | null, args?.expectedSnapshot as Parameters<typeof saveMockDatabaseConnection>[2],
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
  const networkCommands = ["database_connection_test", "database_catalogs_list", "database_schema_get", "database_query_execute",
    "database_table_browse", "database_table_structure", "database_row_mutate", "database_script_execute", "database_export_table"];
  if (args?.expectedConnection && (networkCommands.includes(command) || command === "database_connection_test_input")) {
    const input = (args.input ?? args) as { workspaceId?: string; connectionId?: string; id?: string };
    const record = mockStore.databaseConnections.find((item) => item.workspaceId === input.workspaceId && item.id === (input.connectionId ?? input.id));
    const exp = args.expectedConnection as { revision: number; sshConnectionId: string | null; sshRevision: number | null };
    if (!record || record.revision !== exp.revision || (record.sshConnectionId ?? null) !== exp.sshConnectionId || (record.sshRevision ?? null) !== exp.sshRevision) {
      throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
    }
  }
  if (command === "database_connection_test_input") {
    const input = args?.input as DatabaseConnectionInput;
    const current = mockStore.databaseConnections.find((item) => item.workspaceId === input.workspaceId && item.id === input.id);
    if (resolveMockDatabaseRoute(input, current).sshConnectionId) throw { code: "SSH_TUNNEL_UNAVAILABLE" };
    return { ok: true, message: "Mock connection OK", serverVersion: "mock" } as T;
  }
  if (networkCommands.includes(command)) {
    const input = (args?.input ?? args) as { workspaceId?: string; connectionId?: string };
    const record = mockStore.databaseConnections.find((item) => item.workspaceId === input?.workspaceId && item.id === input?.connectionId);
    if (record?.sshConnectionId) throw { code: "SSH_TUNNEL_UNAVAILABLE" };
  }
  const handler = handlers.get(command);
  // A handled command may return undefined; only a missing handler falls through.
  return handler ? handler(args) as T : UNHANDLED;
}
