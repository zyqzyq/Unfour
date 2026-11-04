import { call, isTauriRuntime } from "./invoke";

export type LocalDataVersion = { workspaceId: string; domain: string; version: string };

/** No native polling in browser previews. Only identifiers and clocks cross IPC. */
export function readLocalDataVersions(): Promise<LocalDataVersion[] | null> {
  return isTauriRuntime() ? call<LocalDataVersion[]>("local_data_versions") : Promise.resolve(null);
}

/** Query owners declare dependencies; runtime queries never opt in. */
export function isLocalDataQuery(meta: Record<string, unknown> | undefined, changes?: LocalDataVersion[]) {
  const domains = meta?.localDataDomains;
  const workspaceId = meta?.cloudSyncWorkspaceId;
  if (!Array.isArray(domains) || (workspaceId !== null && typeof workspaceId !== "string")) return false;
  return changes === undefined || changes.some((change) =>
    (workspaceId === null || workspaceId === change.workspaceId) && domains.includes(change.domain));
}
