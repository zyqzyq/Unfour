import { useEffect, useRef } from "react";
import { listTemporarySsh, onTemporarySshLaunch } from "@unfour/command-client";

export function useExternalSshNavigation(workspaceId: string | undefined, selectModule: (id: string) => void) {
  const select = useRef(selectModule);
  useEffect(() => { select.current = selectModule; }, [selectModule]);
  useEffect(() => {
    if (!workspaceId) return;
    let alive = true;
    let stop: (() => void) | null = null;
    const open = () => { if (alive) select.current("ssh-main"); };
    async function subscribe(id: string) {
      stop = await onTemporarySshLaunch(open);
      if (!alive) { stop?.(); return; }
      // Subscribe before draining the cold-start snapshot so a warm launch can't fall in the gap.
      if (stop && (await listTemporarySsh(id)).some((r) => r.status === "pending")) open();
    }
    void subscribe(workspaceId).catch(() => undefined);
    return () => { alive = false; stop?.(); };
  }, [workspaceId]);
}
