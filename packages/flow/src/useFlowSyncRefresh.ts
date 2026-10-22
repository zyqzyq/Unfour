import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { logFrontendEvent, subscribeCloudSyncWorkspaceChanges } from "@unfour/command-client";

export function useFlowSyncRefresh(workspaceId: string) {
  const queryClient = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let fallback: ReturnType<typeof setInterval> | undefined;
    const refresh = () => {
      if (disposed) return;
      for (const name of ["flows", "flow-resources", "workspace-environments", "flow-workspace-variables"]) {
        const queryKey = [name, workspaceId];
        // Cancel even an initial read: it may have started before the sync commit.
        void queryClient.cancelQueries({ queryKey }).then(() => {
          if (!disposed) return queryClient.invalidateQueries({ queryKey });
        });
      }
    };
    void subscribeCloudSyncWorkspaceChanges((changedWorkspaceId) => {
      if (changedWorkspaceId === workspaceId) refresh();
    }).then((stop) => {
      if (!stop) return;
      if (disposed) { stop(); return; }
      unlisten = stop;
      // Cover commits between the first query and asynchronous listener setup.
      refresh();
    }).catch(() => {
      if (disposed) return;
      void logFrontendEvent({ level: "warn", event: "cloud_sync_listener_failed", module: "flow", operation: "subscribe_workspace_changes" });
      refresh();
      fallback = setInterval(refresh, 15_000);
    });
    return () => {
      disposed = true;
      unlisten?.();
      if (fallback) clearInterval(fallback);
    };
  }, [queryClient, workspaceId]);
}
