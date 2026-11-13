import type { RedisCollectionPage, RedisCollectionRequest, RedisConnection, RedisConnectionInput, RedisKeyInfo, RedisMutationRequest, RedisScanPage } from "../types/redis";
import { call } from "./invoke";
import { bindRedisSession, bindTemporaryHandle, forgetRedisConnectionPasswords, prepareRedisPassword, temporaryRouteArgs } from "./session-routes";
export { bindRedisSession, redisTemporaryPassword } from "./session-routes";

export async function listRedisConnections(workspaceId: string) {
  const records = await call<RedisConnection[]>("redis_connections_list", { workspaceId });
  forgetRedisConnectionPasswords(workspaceId, new Map(records.map((record) => [record.id, record.revision])));
  return records;
}

export async function saveRedisConnection(input: RedisConnectionInput, secret?: string | null, runtime?: { handle: string | null; password?: string }) {
  const ticket = runtime?.handle ? prepareRedisPassword(input.workspaceId, runtime.handle) : undefined;
  try {
    const record = await call<RedisConnection>("redis_connection_save", { input, secret: secret ?? null });
    if (runtime) bindRedisSession(input.workspaceId, record.id, record.revision, runtime.handle, ticket?.valid() ? runtime.password : undefined);
    return record;
  } finally {
    ticket?.release();
  }
}

export function testRedisConnectionInput(input: RedisConnectionInput, secret?: string | null, temporaryHandle?: string) {
  return call<void>("redis_connection_test_input", { input, secret: secret ?? null,
    ...(temporaryHandle && { temporaryHandle }) });
}

export async function deleteRedisConnection(workspaceId: string, connectionId: string, expectedRevision: number) {
  const records = await call<RedisConnection[]>("redis_connection_delete", { workspaceId, connectionId, expectedRevision });
  bindTemporaryHandle("redis", workspaceId, connectionId, null);
  return records;
}

export function inspectRedisKey(workspaceId: string, connectionId: string, keyBase64: string, fieldBase64: string | undefined, expectedRevision: number) {
  return call<RedisKeyInfo>("redis_key_inspect", { workspaceId, connectionId, keyBase64, fieldBase64: fieldBase64 ?? null, expectedRevision, ...temporaryRouteArgs("redis", workspaceId, connectionId, expectedRevision) });
}

export function scanRedisKeys(workspaceId: string, connectionId: string, cursor: string, pattern: string, expectedRevision: number) {
  return call<RedisScanPage>("redis_keys_scan", { workspaceId, connectionId, cursor, pattern, expectedRevision, ...temporaryRouteArgs("redis", workspaceId, connectionId, expectedRevision) });
}

export function browseRedisCollection(request: RedisCollectionRequest) {
  return call<RedisCollectionPage>("redis_collection_browse", { ...request, startIndex: request.startIndex ?? null, ...temporaryRouteArgs("redis", request.workspaceId, request.connectionId, request.expectedRevision) });
}

export function mutateRedisKey(request: RedisMutationRequest) {
  return call<void>("redis_key_mutate", { ...request, ...temporaryRouteArgs("redis", request.workspaceId, request.connectionId, request.expectedRevision) });
}
