import type { RedisConnection, RedisConnectionInput } from "../../types";
import { UNHANDLED, type MockResult } from "./types";

const connections = new Map<string, RedisConnection>();

function save(input: RedisConnectionInput, secret: unknown) {
  const old = input.id ? connections.get(input.id) : undefined;
  if (input.id && (!old || old.workspaceId !== input.workspaceId || old.revision !== input.expectedRevision)) {
    throw { code: "ROW_CONFLICT", message: "Reload the connection before saving." };
  }
  const record: RedisConnection = {
    id: old?.id ?? crypto.randomUUID(), workspaceId: input.workspaceId,
    name: input.name, host: input.host, port: input.port, dbIndex: input.dbIndex,
    tls: input.tls, username: input.username, revision: (old?.revision ?? 0) + 1,
    hasPassword: typeof secret === "string" ? secret.length > 0 : Boolean(old?.hasPassword),
  };
  connections.set(record.id, record);
  return record;
}

export function handleRedisMock<T>(command: string, args?: Record<string, unknown>): MockResult<T> {
  const list = () => Array.from(connections.values()).filter((entry) => entry.workspaceId === args?.workspaceId);
  switch (command) {
    case "redis_connections_list": return list() as T;
    case "redis_connection_save": return save(args?.input as RedisConnectionInput, args?.secret) as T;
    case "redis_connection_delete": {
      const id = String(args?.connectionId);
      const old = connections.get(id);
      if (!old || old.workspaceId !== args?.workspaceId || old.revision !== args?.expectedRevision) {
        throw { code: "ROW_CONFLICT", message: "Reload the connection before deleting." };
      }
      connections.delete(id);
      return list() as T;
    }
    case "redis_connection_test_input":
    case "redis_key_inspect":
    case "redis_keys_scan":
      throw { code: "UNSUPPORTED_OPERATION", message: "Redis network operations require the desktop app." };
    default: return UNHANDLED;
  }
}
