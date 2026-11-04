import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { isCloudSyncQuery, logFrontendEvent, subscribeCloudSyncWorkspaceChanges } from "@unfour/command-client";
import { refreshLocalQueries } from "../local-data/refreshLocalQueries";

/** One desktop subscription refreshes local reads explicitly marked by their owners. */
export function useCloudSyncQueryRefresh() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let fallback: ReturnType<typeof setInterval> | undefined;
    const refresh = (workspaceId?: string) => {
      refreshLocalQueries(queryClient, (query) => isCloudSyncQuery(query.meta, workspaceId), () => disposed);
    };
    void subscribeCloudSyncWorkspaceChanges(refresh).then((stop) => {
      if (!stop) return;
      if (disposed) { stop(); return; }
      unlisten = stop;
      // Cover commits between initial reads and asynchronous listener setup.
      refresh();
    }).catch(() => {
      if (disposed) return;
      void logFrontendEvent({
        level: "warn", event: "cloud_sync_listener_failed",
        module: "cloud-sync", operation: "subscribe_workspace_changes",
      });
      refresh();
      fallback = setInterval(refresh, 15_000);
    });
    return () => {
      disposed = true;
      unlisten?.();
      if (fallback) clearInterval(fallback);
    };
  }, [queryClient]);
}
