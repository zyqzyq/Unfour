import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  registerSshTerminalChannel,
  type SshSessionEvent,
  type SshSessionSummary,
  type SshTerminalDataPayload,
} from "@unfour/command-client";

export function useSshTerminalChannel({
  appendTerminalEvents,
  workspaceId,
}: {
  appendTerminalEvents: (events: SshSessionEvent[]) => void;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    // Live output is coalesced into the store roughly once per frame. It arrives
    // over a Tauri IPC channel (not the event system, which stalls under a
    // full-screen-redraw emit burst on WebView2/Windows) and is batched here so
    // a keystroke echo does not force a full re-render per chunk.
    let pending: SshSessionEvent[] = [];
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flushPending = () => {
      flushTimer = null;
      if (!pending.length) {
        return;
      }
      const batch = pending;
      pending = [];
      appendTerminalEvents(batch);
    };
    const handlePayload = (payload: SshTerminalDataPayload) => {
      if (!payload?.sessionId) {
        return;
      }
      if (payload.data) {
        pending.push({
          sessionId: payload.sessionId,
          kind:
            payload.status === "disconnected" || payload.status === "failed"
              ? "close"
              : "output",
          data: payload.data,
          createdAt: new Date().toISOString(),
        });
        if (flushTimer === null) {
          flushTimer = setTimeout(flushPending, 16);
        }
      }
      if (payload.status) {
        queryClient.setQueryData<SshSessionSummary[]>(
          ["ssh-sessions", workspaceId],
          (current = []) =>
            current.map((session) =>
              session.sessionId === payload.sessionId
                ? {
                    ...session,
                    status: payload.status!,
                    reconnectAttempt: payload.reconnectAttempt ?? 0,
                    updatedAt: new Date().toISOString(),
                  }
                : session,
            ),
        );
      }
    };
    registerSshTerminalChannel(handlePayload)
      .then((d) => {
        if (disposed) {
          d();
        } else {
          dispose = d;
        }
      })
      .catch(() => {
        // Browser mock mode has no Tauri IPC; query polling remains active.
      });
    return () => {
      disposed = true;
      if (flushTimer !== null) {
        clearTimeout(flushTimer);
      }
      flushPending();
      dispose?.();
    };
  }, [appendTerminalEvents, queryClient, workspaceId]);
}
