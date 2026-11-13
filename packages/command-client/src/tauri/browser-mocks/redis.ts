import type { RedisConnection, RedisConnectionInput } from "../../types";
import { UNHANDLED, type MockResult } from "./types";
import { mockStore } from "./state";

const connections = new Map<string, RedisConnection>();

function retainLocalWrites(old: RedisConnection | undefined, input: RedisConnectionInput) {
  return Boolean(old?.allowWrites && old.host === input.host && old.port === input.port
    && old.tls === input.tls && old.dbIndex === input.dbIndex && old.username === input.username
    && (input.sshConnectionId === undefined || (old.sshConnectionId ?? null) === input.sshConnectionId));
}

function resolveRoute(old: RedisConnection | undefined, input: RedisConnectionInput) {
  const id = input.sshConnectionId === undefined ? old?.sshConnectionId ?? null : input.sshConnectionId;
  const jump = mockStore.sshConnections.find((c) => c.workspaceId === input.workspaceId && c.id === id && !c.deletedAt);
  if (id && !jump) throw { code: "SSH_TUNNEL_ROUTE_MISSING" };
  if (input.expectedSshRevision !== undefined && input.expectedSshRevision !== jump?.revision) throw { code: "SSH_TUNNEL_ROUTE_CHANGED" };
  return { sshConnectionId: id, sshConnectionName: jump?.name ?? null };
}

function save(input: RedisConnectionInput, secret: unknown) {
  const old = input.id ? connections.get(input.id) : undefined;
  if (input.id && (!old || old.workspaceId !== input.workspaceId || old.revision !== input.expectedRevision)) {
    throw { code: "ROW_CONFLICT", message: "Reload the connection before saving." };
  }
  const record: RedisConnection = {
    id: old?.id ?? crypto.randomUUID(), workspaceId: input.workspaceId,
    name: input.name, host: input.host, port: input.port, dbIndex: input.dbIndex,
    tls: input.tls, username: input.username, revision: (old?.revision ?? 0) + 1,
    ...resolveRoute(old, input),
    hasPassword: typeof secret === "string" ? secret.length > 0 : Boolean(old?.hasPassword),
    allowWrites: input.allowWrites ?? retainLocalWrites(old, input),
  };
  connections.set(record.id, record);
  return record;
}

export function handleRedisMock<T>(command: string, args: Record<string, unknown> = {}): MockResult<T> {
  const list = () => Array.from(connections.values()).filter((entry) => entry.workspaceId === args.workspaceId);
  switch (command) {
    case "redis_connections_list": return list() as T;
    case "redis_connection_save": return save(args.input as RedisConnectionInput, args.secret) as T;
    case "redis_connection_delete": {
      const id = String(args.connectionId);
      const old = connections.get(id);
      if (!old || old.workspaceId !== args.workspaceId || old.revision !== args.expectedRevision) {
        throw { code: "ROW_CONFLICT", message: "Reload the connection before deleting." };
      }
      connections.delete(id);
      return list() as T;
    }
    case "redis_connection_test_input":
    case "redis_key_inspect":
    case "redis_key_mutate":
    case "redis_collection_browse":
    case "redis_keys_scan":
      throw { code: "UNSUPPORTED_OPERATION", message: "Redis network operations require the desktop app." };
    default: return UNHANDLED;
  }
}
