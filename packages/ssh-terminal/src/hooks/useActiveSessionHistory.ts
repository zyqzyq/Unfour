import { useCallback, useEffect, useRef } from "react";
import {
  getSshSessionHistory,
  type SshSessionEvent,
} from "@unfour/command-client";

export function useActiveSessionHistory({
  active,
  hydrate,
  sessionId,
  workspaceId,
}: {
  active: boolean;
  hydrate: (sessionId: string, events: SshSessionEvent[]) => void;
  sessionId?: string;
  workspaceId: string;
}) {
  const hydratedKeysRef = useRef(new Set<string>());
  const markHydrated = useCallback(
    (nextSessionId: string) => {
      hydratedKeysRef.current.add(`${workspaceId}:${nextSessionId}`);
    },
    [workspaceId],
  );

  useEffect(() => {
    if (!active || !sessionId) return;
    const hydrationKey = `${workspaceId}:${sessionId}`;
    const hydratedKeys = hydratedKeysRef.current;
    if (hydratedKeys.has(hydrationKey)) return;
    hydratedKeys.add(hydrationKey);
    let cancelled = false;
    let completed = false;

    getSshSessionHistory({ workspaceId, sessionId })
      .then((events) => {
        if (cancelled) return;
        hydrate(sessionId, events);
        completed = true;
      })
      .catch(() => hydratedKeys.delete(hydrationKey));

    return () => {
      cancelled = true;
      if (!completed) hydratedKeys.delete(hydrationKey);
    };
  }, [active, hydrate, sessionId, workspaceId]);

  return markHydrated;
}
