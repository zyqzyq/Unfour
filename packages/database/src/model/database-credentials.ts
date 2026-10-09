import {
  type DatabaseConnection,
  type DatabaseConnectionInput,
} from "@unfour/command-client";

export const DATABASE_PASSWORD_KIND = "database-password";

export function emptyDatabaseConnectionForm(
  workspaceId: string,
): DatabaseConnectionInput {
  return { workspaceId, name: "", driver: "sqlite", sqlitePath: "" };
}

/** Map a persisted connection into editor input for the current workspace. */
export function databaseConnectionToInput(
  connection: DatabaseConnection,
  workspaceId: string,
): DatabaseConnectionInput {
  return {
    id: connection.id,
    workspaceId,
    name: connection.name,
    driver: connection.driver,
    host: connection.host,
    port: connection.port,
    database: connection.database,
    username: connection.username,
    sslMode: connection.sslMode,
    sqlitePath: connection.sqlitePath,
    credentialRef: connection.credentialRef,
    readOnly: connection.readOnly,
  };
}
