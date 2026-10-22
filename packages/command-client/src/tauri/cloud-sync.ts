import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./invoke";

/** Only committed local workspace IDs cross the event boundary. */
export async function subscribeCloudSyncWorkspaceChanges(
  onChange: (workspaceId: string) => void,
): Promise<UnlistenFn | null> {
  if (!isTauriRuntime()) return null;
  return listen<{ workspaceId: string }>("cloud-sync://workspace-changed", ({ payload }) => {
    onChange(payload.workspaceId);
  });
}
