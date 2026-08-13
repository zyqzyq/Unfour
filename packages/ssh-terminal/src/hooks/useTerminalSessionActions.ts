import { useState } from "react";
import type { SshSessionSummary } from "@unfour/command-client";
import { shouldCloseTerminalSessionInBackend } from "../model/terminal-tabs";
import type { TerminalSessionTabState } from "../model/types";

type ConnectMutation = {
  mutate: (connectionId: string) => void;
  reset: () => void;
};

type CloseMutation = {
  mutate: (sessionId: string) => void;
};

export function useTerminalSessionActions({
  closeMutation,
  connectMutation,
  dismissSession,
  frontendFailedSessions,
  removeSftpSession,
  sessions,
  sessionTabs,
}: {
  closeMutation: CloseMutation;
  connectMutation: ConnectMutation;
  dismissSession: (sessionId: string) => void;
  frontendFailedSessions: Readonly<Record<string, SshSessionSummary>>;
  removeSftpSession: (sessionId: string) => void;
  sessions: SshSessionSummary[];
  sessionTabs: TerminalSessionTabState[];
}) {
  const [closeConfirmSessionId, setCloseConfirmSessionId] = useState<string | null>(null);

  function closeSessionInBackend(sessionId: string) {
    if (
      !shouldCloseTerminalSessionInBackend({
        frontendFailedSessions,
        sessionId,
      })
    ) {
      connectMutation.reset();
      return;
    }
    closeMutation.mutate(sessionId);
  }

  function requestCloseSession(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    const needsConfirmation =
      session && !["disconnected", "failed"].includes(session.status);
    if (needsConfirmation) {
      setCloseConfirmSessionId(sessionId);
      return;
    }
    // Frontend-only failures have no backend session to close. Backend-managed
    // disconnected/failed sessions still go through the command bus.
    if (session) {
      closeSessionInBackend(sessionId);
    }
    dismissSession(sessionId);
    removeSftpSession(sessionId);
  }

  const closeConfirmSession = closeConfirmSessionId
    ? sessions.find((item) => item.sessionId === closeConfirmSessionId)
    : null;

  // Close a session without the confirmation prompt — used by the batch tab
  // actions (close others/all/left/right) where a dialog per tab would be noise.
  function closeSessionNow(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    if (session) {
      closeSessionInBackend(sessionId);
    }
    dismissSession(sessionId);
    removeSftpSession(sessionId);
  }

  function reconnectSession(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    if (!session) {
      return;
    }
    closeSessionNow(sessionId);
    connectMutation.reset();
    connectMutation.mutate(session.connectionId);
  }

  function closeOtherSessions(sessionId: string) {
    sessionTabs
      .filter((item) => item.session.sessionId !== sessionId)
      .forEach((item) => closeSessionNow(item.session.sessionId));
  }

  function closeAllSessions() {
    sessionTabs.forEach((item) => closeSessionNow(item.session.sessionId));
  }

  function closeSessionsToLeft(sessionId: string) {
    const index = sessionTabs.findIndex((item) => item.session.sessionId === sessionId);
    if (index <= 0) {
      return;
    }
    sessionTabs.slice(0, index).forEach((item) => closeSessionNow(item.session.sessionId));
  }

  function closeSessionsToRight(sessionId: string) {
    const index = sessionTabs.findIndex((item) => item.session.sessionId === sessionId);
    if (index < 0) {
      return;
    }
    sessionTabs.slice(index + 1).forEach((item) => closeSessionNow(item.session.sessionId));
  }

  function confirmCloseSession() {
    if (closeConfirmSessionId) {
      closeSessionInBackend(closeConfirmSessionId);
      dismissSession(closeConfirmSessionId);
      removeSftpSession(closeConfirmSessionId);
    }
    setCloseConfirmSessionId(null);
  }

  return {
    closeAllSessions,
    closeConfirmSession,
    closeConfirmSessionId,
    closeOtherSessions,
    closeSessionsToLeft,
    closeSessionsToRight,
    confirmCloseSession,
    reconnectSession,
    requestCloseSession,
    setCloseConfirmSessionId,
  };
}
