import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { isLocalDataQuery, readLocalDataVersions, type LocalDataVersion } from "@unfour/command-client";
import { refreshLocalQueries } from "./refreshLocalQueries";

const POLL_INTERVAL_MS = 2_000;
const key = (value: LocalDataVersion) => JSON.stringify([value.workspaceId, value.domain]);

/** One content-free clock read per visible desktop, no overlapping requests. */
export function useLocalDataQueryRefresh() {
  const client = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let pending = false;
    let native = true;
    let previous: Map<string, LocalDataVersion> | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const check = async () => {
      if (disposed || pending || !native || document.visibilityState === "hidden") return;
      if (timer) clearTimeout(timer);
      pending = true;
      try {
        const versions = await readLocalDataVersions();
        if (disposed) return;
        if (versions === null) { native = false; return; }
        const current = new Map(versions.map((value) => [key(value), value]));
        const changes = previous ? [
          ...versions.filter((value) => previous?.get(key(value))?.version !== value.version),
          ...[...previous.values()].filter((value) => !current.has(key(value))),
        ] : undefined;
        previous = current;
        // First snapshot also covers commits between initial query reads and setup.
        refreshLocalQueries(client, (query) => isLocalDataQuery(query.meta, changes) ||
          (query.state.status === "error" && query.state.isInvalidated && isLocalDataQuery(query.meta)), () => disposed);
      } catch {
        // Keep the last successful clocks. A transient SQLite/IPC failure must
        // not acknowledge changes; retry on the next tick or window activation.
      } finally {
        pending = false;
        if (!disposed && native) timer = setTimeout(() => { void check(); }, POLL_INTERVAL_MS);
      }
    };
    const wake = () => { void check(); };
    document.addEventListener("visibilitychange", wake);
    window.addEventListener("focus", wake);
    wake();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
      document.removeEventListener("visibilitychange", wake);
      window.removeEventListener("focus", wake);
    };
  }, [client]);
}
