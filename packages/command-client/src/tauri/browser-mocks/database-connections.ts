import type { DatabaseConnection, DatabaseConnectionInput, DatabaseTestResult } from "../../types";
import { mockStore } from "./state";
import { connectionEditSnapshot } from "./editor-snapshots";

export function listMockDatabaseConnections(workspaceId: string) {
  return mockStore.databaseConnections.filter(
    (item) => item.workspaceId === workspaceId,
  );
}

export function saveMockDatabaseConnection(input: DatabaseConnectionInput, secret?: string | null, expectedSnapshot?: DatabaseConnection) {
  const old = mockStore.databaseConnections.find((item) => item.workspaceId === input.workspaceId && item.id === input.id);
  const route = resolveMockDatabaseRoute(input, old);
  if (expectedSnapshot && connectionEditSnapshot(expectedSnapshot) !== connectionEditSnapshot(
    mockStore.databaseConnections.find((item) => item.id === input.id && item.workspaceId === input.workspaceId),
  )) throw { code: "ROW_CONFLICT", message: "record changed externally; reload before saving" };
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
    ...route,
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

export function resolveMockDatabaseRoute(input: DatabaseConnectionInput, old?: DatabaseConnection) {
  const sshConnectionId = input.sshConnectionId === undefined ? old?.sshConnectionId ?? null : input.sshConnectionId;
  if (!sshConnectionId) return { sshConnectionId: null, sshRevision: null };
  if (input.driver === "sqlite") throw { code: "SSH_TUNNEL_INVALID_ROUTE" };
  const host = mockStore.sshConnections.find((c) => c.workspaceId === input.workspaceId && c.id === sshConnectionId && !c.deletedAt);
  if (!host) throw { code: "SSH_TUNNEL_ROUTE_MISSING" };
  if (input.expectedSshRevision != null && host.revision !== input.expectedSshRevision) throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
  return { sshConnectionId, sshRevision: input.sshConnectionId === undefined ? old?.sshRevision : host.revision };
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
