import type { DatabaseConnection, DatabaseConnectionInput, DatabaseTestResult } from "../../types";
import { mockStore } from "./state";

export function listMockDatabaseConnections(workspaceId: string) {
  return mockStore.databaseConnections.filter(
    (item) => item.workspaceId === workspaceId,
  );
}

export function saveMockDatabaseConnection(input: DatabaseConnectionInput, secret?: string | null) {
  let credentialRef = input.driver === "sqlite" ? null : input.credentialRef ?? null;
  if (input.driver !== "sqlite" && secret) {
    credentialRef = `unfour:${input.workspaceId}:database-password:${crypto.randomUUID()}`;
    mockStore.credentials[credentialRef] = secret;
  }
  const now = new Date().toISOString();
  const existingIndex = input.id
    ? mockStore.databaseConnections.findIndex((item) => item.id === input.id)
    : -1;
  const connection: DatabaseConnection = {
    id: input.id || crypto.randomUUID(),
    workspaceId: input.workspaceId,
    name: input.name,
    driver: input.driver,
    ...connectionSettings(input),
    credentialRef,
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
  return connection;
}

function connectionSettings(input: DatabaseConnectionInput) {
  return {
    host: input.host ?? null,
    port: input.port ?? null,
    database: input.database ?? null,
    username: input.username ?? null,
    sslMode: input.sslMode ?? null,
    sqlitePath: input.sqlitePath ?? null,
    readOnly: input.readOnly ?? false,
  };
}

export function deleteMockDatabaseConnection(workspaceId: string, connectionId: string) {
  mockStore.databaseConnections = mockStore.databaseConnections.filter(
    (item) => !(item.workspaceId === workspaceId && item.id === connectionId),
  );
  return mockStore.databaseConnections.filter(
    (item) => item.workspaceId === workspaceId,
  );
}

export function testMockDatabaseConnection(connectionId: string) {
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
  } satisfies DatabaseTestResult);
}
