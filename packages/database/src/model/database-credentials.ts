import {
  connectionTemporaryHandle,
  type DatabaseConnection,
  type DatabaseConnectionInput,
} from "@unfour/command-client";

export const DATABASE_PASSWORD_KIND = "database-password";

export function databaseConnectionContent(connection: DatabaseConnection | null) {
  return connection ? JSON.stringify({ ...databaseConnectionToInput(connection, connection.workspaceId), expectedSnapshot: undefined }) : null;
}

export function emptyDatabaseConnectionForm(
  workspaceId: string,
): DatabaseConnectionInput {
  return { workspaceId, name: "", driver: "sqlite", sqlitePath: "" };
}

export function onlyTemporaryRouteChanged(input: DatabaseConnectionInput, secret: string) {
  const saved = input.expectedSnapshot;
  if (!saved || secret || input.temporaryHandle === undefined) return false;
  const original = databaseConnectionToInput(saved, input.workspaceId);
  const fields = ["id", "name", "driver", "host", "port", "database", "username", "sslMode",
    "sqlitePath", "credentialRef", "readOnly", "sshConnectionId", "expectedSshRevision"] as const;
  return fields.every((key) => (input[key] ?? null) === (original[key] ?? null));
}

/** Map a persisted connection into editor input for the current workspace. */
export function databaseConnectionToInput(
  connection: DatabaseConnection,
  workspaceId: string,
): DatabaseConnectionInput {
  return {
    id: connection.id,
    expectedSnapshot: connection,
    sshConnectionId: connection.sshConnectionId,
    expectedSshRevision: connection.sshRevision,
    temporaryHandle: connectionTemporaryHandle("database", workspaceId, connection.id),
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
