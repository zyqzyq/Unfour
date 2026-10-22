import { useEffect } from "react";
import { useQueryClient, type Query } from "@tanstack/react-query";
import { isCloudSyncQuery, logFrontendEvent, subscribeCloudSyncWorkspaceChanges } from "@unfour/command-client";

/** One desktop subscription refreshes local reads explicitly marked by their owners. */
export function useCloudSyncQueryRefresh() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let fallback: ReturnType<typeof setInterval> | undefined;
    const refresh = (workspaceId?: string) => {
      if (disposed) return;
      const filters = { predicate: (query: Query) => isCloudSyncQuery(query.meta, workspaceId) };
      const pending = queryClient.getQueryCache().findAll(filters)
        .filter((query) => query.state.fetchStatus === "fetching")
        .map((query) => ({ query, promise: query.promise }));
      void queryClient.invalidateQueries(filters, { cancelRefetch: false });
      // A query can also serve an imperative fetch (e.g. opening a task).
      // Let that caller finish, then reread data that may predate the commit.
      for (const { query, promise } of pending) {
        void Promise.resolve(promise).catch(() => undefined).then(() => {
          if (!disposed) return queryClient.invalidateQueries(
            { predicate: (candidate) => candidate === query }, { cancelRefetch: false },
          );
        });
      }
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
