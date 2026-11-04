import type { Query, QueryClient } from "@tanstack/react-query";

/** Shared by Cloud Sync events and cross-process local commit detection. */
export function refreshLocalQueries(client: QueryClient, predicate: (query: Query) => boolean, disposed: () => boolean) {
  if (disposed()) return;
  const filters = { predicate };
  const pending = client.getQueryCache().findAll(filters)
    .filter((query) => query.state.fetchStatus === "fetching")
    .map((query) => ({ query, promise: query.promise }));
  void client.invalidateQueries(filters, { cancelRefetch: false });
  // Never cancel an imperative detail fetch. Its result may predate this commit;
  // mark it stale again after it finishes, or refetch if it has active observers.
  for (const { query, promise } of pending) {
    void Promise.resolve(promise).catch(() => undefined).then(() => {
      if (!disposed()) return client.invalidateQueries(
        { predicate: (candidate) => candidate === query }, { cancelRefetch: false },
      );
    });
  }
}
