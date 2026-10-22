import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./invoke";

/** Query owners opt local reads into refresh; null denotes a global workspace list. */
export type CloudSyncQueryMeta = { cloudSyncWorkspaceId: string | null };

export function isCloudSyncQuery(
  meta: Record<string, unknown> | undefined,
  workspaceId?: string,
): boolean {
  const scope = meta?.cloudSyncWorkspaceId;
  return scope === null || (typeof scope === "string" && (workspaceId === undefined || scope === workspaceId));
}

/** Only committed local workspace IDs cross the event boundary. */
export async function subscribeCloudSyncWorkspaceChanges(
  onChange: (workspaceId: string) => void,
): Promise<UnlistenFn | null> {
  if (!isTauriRuntime()) return null;
  return listen<{ workspaceId: string }>("cloud-sync://workspace-changed", ({ payload }) => {
    onChange(payload.workspaceId);
  });
}
