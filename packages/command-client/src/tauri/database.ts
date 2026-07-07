import { call } from "./invoke";
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
  DbQueryHistoryEntry,
  SavedSql,
  SavedSqlInput,
} from "../types";

export function listDatabaseConnections(workspaceId: string) {
  return call<DatabaseConnection[]>("database_connections_list", { workspaceId });
}

export function saveDatabaseConnection(input: DatabaseConnectionInput) {
  return call<DatabaseConnection>("database_connection_save", { input });
}

export function deleteDatabaseConnection(workspaceId: string, connectionId: string) {
  return call<DatabaseConnection[]>("database_connection_delete", {
    workspaceId,
    connectionId,
  });
}

export function testDatabaseConnection(workspaceId: string, connectionId: string) {
  return call<DatabaseTestResult>("database_connection_test", {
    workspaceId,
    connectionId,
  });
}

export function testDatabaseConnectionInput(input: DatabaseConnectionInput, secret: string | null) {
  return call<DatabaseTestResult>("database_connection_test_input", {
    input,
    secret,
  });
}

export function getDatabaseSchema(
  workspaceId: string,
  connectionId: string,
  catalog?: string | null,
) {
  return call<DatabaseSchema>("database_schema_get", {
    workspaceId,
    connectionId,
    catalog: catalog ?? null,
  });
}

export function listDatabaseCatalogs(workspaceId: string, connectionId: string) {
  return call<string[]>("database_catalogs_list", {
    workspaceId,
    connectionId,
  });
}

export function executeDatabaseQuery(input: DatabaseQueryInput) {
  return call<DatabaseQueryResult>("database_query_execute", { input });
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
  return call<DatabaseBrowseResult>("database_table_browse", { input });
}

export function getDatabaseTableStructure(input: DatabaseTableStructureInput) {
  return call<DatabaseTableStructure>("database_table_structure", { input });
}

export function mutateDatabaseRow(input: DatabaseRowMutationInput) {
  return call<DatabaseRowMutationResult>("database_row_mutate", { input });
}
