/** Runtime-only bindings for actions on a connection selected in this desktop window. */
type ConnectionModule = "database" | "redis" | "ssh";
const handles = new Map<string, string>();
const redisPasswords = new Map<string, { value: string; revision: number }>();
const sqlRouteVersions = new Map<string, number>();
const liveRedisSources = new Map<string, Set<string>>();
const pendingRedisPasswords = new Set<{ workspace: string; handle: string; valid: boolean }>();

export function prepareRedisPassword(workspace: string, handle: string) {
  const ticket = { workspace, handle, valid: true };
  pendingRedisPasswords.add(ticket);
  return { valid: () => ticket.valid, release: () => pendingRedisPasswords.delete(ticket) };
}

export function observeLiveRedisSources(workspace: string, live: Set<string>) {
  liveRedisSources.set(workspace, live);
  for (const ticket of pendingRedisPasswords) if (ticket.workspace === workspace && !live.has(ticket.handle)) ticket.valid = false;
  for (const id of redisPasswords.keys()) {
    const [, owner] = JSON.parse(id) as [ConnectionModule, string, string];
    if (owner === workspace && !live.has(handles.get(id)!)) redisPasswords.delete(id);
  }
}

export function forgetRedisConnectionPasswords(workspace: string, revisions: Map<string, number>) {
  for (const [id, credential] of redisPasswords) {
    const [, owner, connection] = JSON.parse(id) as [ConnectionModule, string, string];
    if (owner === workspace && revisions.get(connection) !== credential.revision) redisPasswords.delete(id);
  }
}

function key(module: ConnectionModule, workspaceId: string, connectionId: string) {
  return JSON.stringify([module, workspaceId, connectionId]);
}

export function connectionTemporaryHandle(module: ConnectionModule, workspaceId: string, connectionId?: string | null) {
  return connectionId ? handles.get(key(module, workspaceId, connectionId)) : undefined;
}

export function bindTemporaryHandle(module: ConnectionModule, workspaceId: string, connectionId: string, handle?: string | null) {
  const id = key(module, workspaceId, connectionId);
  if (module === "database" && handles.get(id) !== (handle ?? undefined)) sqlRouteVersions.set(id, (sqlRouteVersions.get(id) ?? 0) + 1);
  if (handle) handles.set(id, handle);
  else handles.delete(id);
  if (module === "redis") redisPasswords.delete(id);
}

export function databaseRouteVersion(workspaceId: string, connectionId?: string | null) {
  return sqlRouteVersions.get(key("database", workspaceId, connectionId ?? "")) ?? 0;
}

export function invalidateSavedDatabaseRoute(workspaceId: string, connectionId: string) {
  const id = key("database", workspaceId, connectionId);
  sqlRouteVersions.set(id, (sqlRouteVersions.get(id) ?? 0) + 1);
}

export function bindRedisSession(workspaceId: string, connectionId: string, revision: number, handle: string | null, password?: string) {
  bindTemporaryHandle("redis", workspaceId, connectionId, handle);
  const live = liveRedisSources.get(workspaceId);
  if (handle && password && (!live || live.has(handle))) redisPasswords.set(key("redis", workspaceId, connectionId), { value: password, revision });
}

export function redisTemporaryPassword(workspaceId: string, connectionId: string, revision: number) {
  const id = key("redis", workspaceId, connectionId);
  const entry = redisPasswords.get(id);
  if (!entry || entry.revision !== revision) return undefined;
  return entry.value;
}

/** Keep expired selections visible so they fail closed, but release their passwords. */
export function forgetTemporaryPasswords(workspaceId: string, handle?: string) {
  const live = liveRedisSources.get(workspaceId);
  if (handle) live?.delete(handle);
  else liveRedisSources.set(workspaceId, new Set());
  for (const ticket of pendingRedisPasswords) if (ticket.workspace === workspaceId && (!handle || handle === ticket.handle)) ticket.valid = false;
  for (const id of redisPasswords.keys()) {
    const [, workspace] = JSON.parse(id) as [ConnectionModule, string, string];
    if (workspace === workspaceId && (!handle || handles.get(id) === handle)) redisPasswords.delete(id);
  }
}

export function temporaryRouteArgs(module: ConnectionModule, workspaceId: string, connectionId: string, revision?: number) {
  const temporaryHandle = connectionTemporaryHandle(module, workspaceId, connectionId);
  if (!temporaryHandle) return {};
  const password = module === "redis" && revision !== undefined ? redisTemporaryPassword(workspaceId, connectionId, revision) : undefined;
  return { temporaryHandle, ...(password && { temporaryRedisPassword: password }) };
}
