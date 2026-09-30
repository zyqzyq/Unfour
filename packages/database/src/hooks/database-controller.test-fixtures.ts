import type { DatabaseConnection, DatabaseSchema, DatabaseTable } from "@unfour/command-client";

export function connection(id: string, driver: DatabaseConnection["driver"]): DatabaseConnection {
  return {
    id, driver, workspaceId: "ws", name: id, database: driver === "sqlite" ? null : "app",
    host: "localhost", port: 5432, username: "user", sslMode: null, sqlitePath: "app.sqlite",
    credentialRef: null, readOnly: false, createdAt: "now", updatedAt: "now",
    deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
  };
}

export function table(catalog: string | null, name = "users", schema: string | null = null): DatabaseTable {
  return {
    catalog, schema, name, kind: "table",
    columns: [{ name: "id", dataType: "integer", nullable: false, primaryKey: true }],
  };
}

export function schema(connectionId: string, ...tables: DatabaseTable[]): DatabaseSchema {
  return { connectionId, tables };
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
