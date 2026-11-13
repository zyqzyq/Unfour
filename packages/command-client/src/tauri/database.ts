import { bindTemporaryHandle, databaseRouteVersion, invalidateSavedDatabaseRoute, temporaryRouteArgs } from "./session-routes";
export { databaseRouteVersion } from "./session-routes";
import { call } from "./invoke";
import type {
  DatabaseActionContext,
  DatabaseConnectionStamp,
  DatabaseBrowseInput,
  DatabaseBrowseResult,
  DatabaseExportTableInput,
  DatabaseExportTableResult,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryInput,
  DatabaseQueryResult,
  DatabaseScriptInput,
  DatabaseScriptResult,
  DatabaseRowMutationInput,
  DatabaseRowMutationResult,
  DatabaseSchema,
  DatabaseTableStructure,
  DatabaseTableStructureInput,
  DatabaseTestResult,
  DbQueryHistoryEntry,
  SavedSql,
  SavedSqlInput,
} from "../types";

const savedTargets = new Map<string, Map<string, DatabaseConnectionStamp>>();

export function databaseConnectionStamp(record: DatabaseConnection): DatabaseConnectionStamp {
  return { revision: record.revision, sshConnectionId: record.sshConnectionId ?? null, sshRevision: record.sshRevision ?? null };
}

export function databaseActionContext(workspaceId: string, connectionId: string, displayed?: DatabaseConnection): DatabaseActionContext {
  return {
    expectedConnection: displayed ? databaseConnectionStamp(displayed) : savedTargets.get(workspaceId)?.get(connectionId),
    routeVersion: databaseRouteVersion(workspaceId, connectionId),
  };
}

function observeConnection(record: DatabaseConnection) {
  let records = savedTargets.get(record.workspaceId);
  if (!records) savedTargets.set(record.workspaceId, records = new Map());
  const stamp = databaseConnectionStamp(record);
  const old = records.get(record.id);
  if (old && stamp.revision < old.revision) return;
  if (old && JSON.stringify(old) !== JSON.stringify(stamp)) invalidateSavedDatabaseRoute(record.workspaceId, record.id);
  records.set(record.id, stamp);
}

export async function listDatabaseConnections(workspaceId: string) {
  const records = await call<DatabaseConnection[]>("database_connections_list", { workspaceId });
  for (const record of records) observeConnection(record);
  return records;
}

export async function saveDatabaseConnection(input: DatabaseConnectionInput, secret?: string | null) {
  const { expectedSnapshot, temporaryHandle, ...payload } = input;
  const saved = await call<DatabaseConnection>("database_connection_save", { input: payload, secret: secret ?? null, expectedSnapshot });
  observeConnection(saved);
  if (temporaryHandle !== undefined) bindTemporaryHandle("database", input.workspaceId, saved.id, temporaryHandle);
  return saved;
}

export async function deleteDatabaseConnection(workspaceId: string, connectionId: string) {
  const remaining = await call<DatabaseConnection[]>("database_connection_delete", {
    workspaceId,
    connectionId,
  });
  bindTemporaryHandle("database", workspaceId, connectionId, null);
  savedTargets.get(workspaceId)?.delete(connectionId);
  return remaining;
}

function operationArgs(workspaceId: string, connectionId: string, context?: DatabaseActionContext) {
  if (context?.routeVersion != null && databaseRouteVersion(workspaceId, connectionId) !== context.routeVersion) {
    throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
  }
  const expectedConnection = context?.expectedConnection ?? savedTargets.get(workspaceId)?.get(connectionId);
  return { ...temporaryRouteArgs("database", workspaceId, connectionId), ...(expectedConnection && { expectedConnection }) };
}

function callDatabaseAction<T, I extends DatabaseActionContext & { workspaceId: string; connectionId: string }>(command: string, action: I) {
  const { expectedConnection, routeVersion, ...input } = action;
  return call<T>(command, { input, ...operationArgs(action.workspaceId, action.connectionId, { expectedConnection, routeVersion }) });
}

export function testDatabaseConnection(workspaceId: string, connectionId: string, context?: DatabaseActionContext) {
  return call<DatabaseTestResult>("database_connection_test", {
    workspaceId,
    connectionId,
    ...operationArgs(workspaceId, connectionId, context),
  });
}

export function testDatabaseConnectionInput(input: DatabaseConnectionInput, secret: string | null) {
  const { temporaryHandle, expectedSnapshot, ...payload } = input;
  return call<DatabaseTestResult>("database_connection_test_input", {
    input: payload, ...(temporaryHandle && { temporaryHandle }),
    ...(expectedSnapshot && { expectedConnection: databaseConnectionStamp(expectedSnapshot) }),
    secret,
  });
}

export function getDatabaseSchema(
  workspaceId: string,
  connectionId: string,
  catalog?: string | null,
  context?: DatabaseActionContext,
) {
  return call<DatabaseSchema>("database_schema_get", {
    workspaceId,
    connectionId,
    ...operationArgs(workspaceId, connectionId, context),
    catalog: catalog ?? null,
  });
}

export function listDatabaseCatalogs(workspaceId: string, connectionId: string, context?: DatabaseActionContext) {
  return call<string[]>("database_catalogs_list", {
    workspaceId,
    connectionId,
    ...operationArgs(workspaceId, connectionId, context),
  });
}

export function executeDatabaseQuery(input: DatabaseQueryInput) {
  return callDatabaseAction<DatabaseQueryResult, typeof input>("database_query_execute", input);
}

export function executeDatabaseScript(input: DatabaseScriptInput) {
  return callDatabaseAction<DatabaseScriptResult, typeof input>("database_script_execute", input);
}

export function stopDatabaseScript(workspaceId: string, runId: string) {
  return call<boolean>("database_script_stop", { workspaceId, runId });
}

export function recordDatabaseQueryHistory(input: DbQueryHistoryEntry) {
  return call<void>("database_query_history_record", { input });
}

export function listDatabaseQueryHistory(workspaceId: string, limit = 200) {
  return call<DbQueryHistoryEntry[]>("database_query_history_list", { workspaceId, limit });
}

export function clearDatabaseQueryHistory(workspaceId: string) {
  return call<void>("database_query_history_clear", { workspaceId });
}

export function listSavedSql(workspaceId: string) {
  return call<SavedSql[]>("database_saved_sql_list", { workspaceId });
}

export function saveSavedSql(input: SavedSqlInput) {
  return call<SavedSql>("database_saved_sql_save", { input });
}

export function deleteSavedSql(workspaceId: string, id: string) {
  return call<SavedSql[]>("database_saved_sql_delete", { workspaceId, id });
}

export function browseDatabaseTable(input: DatabaseBrowseInput) {
  return callDatabaseAction<DatabaseBrowseResult, typeof input>("database_table_browse", input);
}

export function getDatabaseTableStructure(input: DatabaseTableStructureInput) {
  return callDatabaseAction<DatabaseTableStructure, typeof input>("database_table_structure", input);
}

export function exportDatabaseTable(input: DatabaseExportTableInput) {
  return callDatabaseAction<DatabaseExportTableResult, typeof input>("database_export_table", input);
}

export function mutateDatabaseRow(input: DatabaseRowMutationInput) {
  return callDatabaseAction<DatabaseRowMutationResult, typeof input>("database_row_mutate", input);
}
