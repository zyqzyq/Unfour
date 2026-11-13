import type { RedisCollectionPage, RedisCollectionRequest, RedisConnection, RedisConnectionInput, RedisKeyInfo, RedisMutationRequest, RedisScanPage } from "../types/redis";
import { call } from "./invoke";

export function listRedisConnections(workspaceId: string) {
  return call<RedisConnection[]>("redis_connections_list", { workspaceId });
}

export function saveRedisConnection(input: RedisConnectionInput, secret?: string | null) {
  return call<RedisConnection>("redis_connection_save", { input, secret: secret ?? null });
}

export function testRedisConnectionInput(input: RedisConnectionInput, secret?: string | null) {
  return call<void>("redis_connection_test_input", { input, secret: secret ?? null });
}

export function deleteRedisConnection(workspaceId: string, connectionId: string, expectedRevision: number) {
  return call<RedisConnection[]>("redis_connection_delete", { workspaceId, connectionId, expectedRevision });
}

export function inspectRedisKey(workspaceId: string, connectionId: string, keyBase64: string, fieldBase64: string | undefined, expectedRevision: number) {
  return call<RedisKeyInfo>("redis_key_inspect", { workspaceId, connectionId, keyBase64, fieldBase64: fieldBase64 ?? null, expectedRevision });
}

export function scanRedisKeys(workspaceId: string, connectionId: string, cursor: string, pattern: string, expectedRevision: number) {
  return call<RedisScanPage>("redis_keys_scan", { workspaceId, connectionId, cursor, pattern, expectedRevision });
}

export function browseRedisCollection(request: RedisCollectionRequest) {
  return call<RedisCollectionPage>("redis_collection_browse", { ...request, startIndex: request.startIndex ?? null });
}

export function mutateRedisKey(request: RedisMutationRequest) {
  return call<void>("redis_key_mutate", { ...request });
}
