import { useMemo, useRef, type RefObject } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  closeSshSession,
  connectSshSession,
  exportSshLog,
  type SshConnection,
  type SshSessionEvent,
  type SshSessionSummary,
} from "@unfour/command-client";
import { useFeedback, useI18n } from "@unfour/ui";
import { useActiveSessionHistory } from "./useActiveSessionHistory";
import { useSshTerminalChannel } from "./useSshTerminalChannel";
import { useTerminalSessionActions } from "./useTerminalSessionActions";
import { useTerminalSessions } from "./useTerminalSessions";
import { useSftpStore } from "../model/sftp-state";
import { useTerminalStore } from "../model/terminal-state";
import { buildTerminalSessionTabs, shouldShowTerminalSessionTab } from "../model/terminal-tabs";
import { formatTerminalError } from "../model/errors";

/** Coordinates saved sessions; the page owns workspace activation and selection reconciliation. */
export function useTerminalSessionOrchestration({
  active,
  connections,
  workspaceId,
  workspaceScope,
}: {
  active: boolean;
  connections: SshConnection[];
  workspaceId: string;
  workspaceScope: RefObject<string | null>;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const feedback = useFeedback();
  const activeSessionId = useTerminalStore((state) => state.activeSessionId);
  const addFrontendFailedSession = useTerminalStore(
    (state) => state.addFrontendFailedSession,
  );
  const appendTerminalEvents = useTerminalStore((state) => state.appendTerminalEvents);
  const clearTerminalSessionEvents = useTerminalStore(
    (state) => state.clearTerminalSessionEvents,
  );
  const dismissSession = useTerminalStore((state) => state.dismissSession);
  const removeSftpSession = useSftpStore((state) => state.removeSession);
  const dismissedSessionIds = useTerminalStore((state) => state.dismissedSessionIds);
  const frontendFailedSessions = useTerminalStore(
    (state) => state.frontendFailedSessions,
  );
  const hydrateTerminalSession = useTerminalStore(
    (state) => state.hydrateTerminalSession,
  );
  const setActiveSessionId = useTerminalStore((state) => state.setActiveSessionId);
  const setExportedLog = useTerminalStore((state) => state.setExportedLog);
  const startTerminalSession = useTerminalStore((state) => state.startTerminalSession);
  // While Tasks keeps this page mounted but hidden, freeze the events snapshot so
  // live PTY traffic does not re-render the Connections tree / xterm panes.
  const frozenTerminalEventsRef = useRef<SshSessionEvent[]>([]);
  const terminalEvents = useTerminalStore((state) => {
    if (!active) {
      return frozenTerminalEventsRef.current;
    }
    frozenTerminalEventsRef.current = state.terminalEvents;
    return state.terminalEvents;
  });
  const sessionsQuery = useTerminalSessions(workspaceId, { active });
  const backendSessions = useMemo(() => sessionsQuery.data ?? [], [sessionsQuery.data]);
  // Merge frontend-only failed sessions (created when connect fails before the
  // backend can produce a session record) with the backend session list.
  const sessions = useMemo(() => {
    const failed = Object.values(frontendFailedSessions);
    if (failed.length === 0) return backendSessions;
    // Auto-clean: drop frontend failed entries whose connectionId already has a
    // real backend session (e.g. after a successful retry).
    const backendConnectionIds = new Set(backendSessions.map((s) => s.connectionId));
    const surviving = failed.filter(
      (f) => !backendConnectionIds.has(f.connectionId),
    );
    return [...backendSessions, ...surviving];
  }, [backendSessions, frontendFailedSessions]);
  // The backend returns disconnected sessions as history. Keep the tab strip focused on
  // active work, while preserving the currently selected session if it disconnects.
  const visibleSessions = useMemo(
    () =>
      sessions
        .filter((session) =>
          shouldShowTerminalSessionTab({ activeSessionId, dismissedSessionIds, session }),
        )
        // The backend lists sessions by updated_at DESC, so the active session's
        // tab would jump to the front on every 2s poll as its activity timestamp
        // advances — making the highlighted tab appear to switch on its own. Pin
        // the tab strip to a stable creation order instead.
        .sort(
          (a, b) =>
            a.createdAt.localeCompare(b.createdAt) ||
            a.sessionId.localeCompare(b.sessionId),
        ),
    [activeSessionId, dismissedSessionIds, sessions],
  );
  const activeSession = useMemo(
    () => visibleSessions.find((item) => item.sessionId === activeSessionId) ?? null,
    [activeSessionId, visibleSessions],
  );
  const sessionTabs = useMemo(
    () => buildTerminalSessionTabs({ connections, sessions: visibleSessions }),
    [connections, visibleSessions],
  );

  useSshTerminalChannel({
    appendTerminalEvents,
    workspaceId,
  });
  const markSessionHistoryHydrated = useActiveSessionHistory({
    active,
    hydrate: hydrateTerminalSession,
    sessionId: activeSession?.sessionId,
    workspaceId,
  });

  const connectMutation = useMutation({
    mutationFn: (request: string | { connectionId: string; sourceSessionId: string }) => {
      const connectionId = typeof request === "string" ? request : request.connectionId;
      const input = { workspaceId, connectionId, cols: 120, rows: 32 };
      return typeof request === "string" ? connectSshSession(input) : connectSshSession(input, request.sourceSessionId);
    },
    onSuccess: (session) => {
      if (workspaceScope.current !== session.workspaceId) {
        void queryClient.invalidateQueries({ queryKey: ["ssh-sessions", session.workspaceId] });
        return;
      }
      if (session.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
        feedback.info(t("feedback.activityRecordFailed"));
      }
      markSessionHistoryHydrated(session.sessionId);
      startTerminalSession(session.sessionId, [
        {
          sessionId: session.sessionId,
          kind: "output",
          data: `${t("ssh.session.connected", {
            host: session.host,
            username: session.username,
          })}\r\n`,
          createdAt: session.createdAt,
        },
      ]);
      queryClient.setQueryData<SshSessionSummary[]>(
        ["ssh-sessions", workspaceId],
        (current = []) => [
          ...current.filter((item) => item.sessionId !== session.sessionId),
          session,
        ],
      );
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
    onError: (error, request) => {
      if (workspaceScope.current !== workspaceId) return;
      if (typeof request !== "string") {
        feedback.error(formatTerminalError(error, t));
        return;
      }
      const connectionId = request;
      const connection = connections.find((c) => c.id === connectionId);
      if (!connection) return;
      const syntheticId = `__frontend_failed_${connectionId}_${Date.now()}`;
      const now = new Date().toISOString();
      const failedSession: SshSessionSummary = {
        sessionId: syntheticId,
        workspaceId,
        connectionId,
        status: "failed",
        reconnectAttempt: 0,
        authKind: connection.authKind,
        host: connection.host,
        username: connection.username,
        cols: 120,
        rows: 32,
        createdAt: now,
        updatedAt: now,
      };
      const errorMessage = formatTerminalError(error, t);
      startTerminalSession(syntheticId, [
        {
          sessionId: syntheticId,
          kind: "output",
          data: `\x1b[31mConnection failed: ${errorMessage}\x1b[0m\r\n`,
          createdAt: now,
        },
      ]);
      addFrontendFailedSession(failedSession);
    },
  });
  const closeMutation = useMutation({
    mutationFn: (sessionId: string) => closeSshSession({ workspaceId, sessionId }),
    onSuccess: (session) => {
      if (workspaceScope.current !== session.workspaceId) {
        void queryClient.invalidateQueries({ queryKey: ["ssh-sessions", session.workspaceId] });
        return;
      }
      if (session.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
        feedback.info(t("feedback.activityRecordFailed"));
      }
      const historyWarning = session.warnings?.includes("SSH_HISTORY_RECORD_FAILED")
        ? t("feedback.ssh.historyRecordFailed") : null;
      if (historyWarning) feedback.info(historyWarning);
      appendTerminalEvents([
        {
          sessionId: session.sessionId,
          kind: "close",
          data: `${t("ssh.session.closed")}\r\n${historyWarning ? `${historyWarning}\r\n` : ""}`,
          createdAt: session.updatedAt,
        },
      ]);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });

  const exportMutation = useMutation({
    mutationFn: (sessionId: string) => exportSshLog({ workspaceId, sessionId }),
    onSuccess: (log) => setExportedLog(log.content),
  });

  function openFromSession(connectionId: string, sourceSessionId: string) {
    connectMutation.reset();
    connectMutation.mutate(frontendFailedSessions[sourceSessionId] ? connectionId : { connectionId, sourceSessionId });
  }

  const actions = useTerminalSessionActions({
    closeMutation,
    connectMutation,
    dismissSession,
    frontendFailedSessions,
    removeSftpSession,
    sessions,
    sessionTabs,
  });

  return {
    ...actions,
    activeSession,
    activeSessionId,
    actionError: connectMutation.error ?? closeMutation.error ?? exportMutation.error,
    clearTerminalSessionEvents,
    closeMutation,
    connectMutation,
    openFromSession,
    sessionsQuery,
    sessionTabs,
    setActiveSessionId,
    terminalEvents,
    visibleSessions,
  };
}
