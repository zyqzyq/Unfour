import { useState } from "react";
import type { SshSessionSummary } from "@unfour/command-client";
import {
  shouldCloseTerminalSessionInBackend,
  terminalBatchCloseTabs,
  type TerminalBatchCloseKind,
} from "../model/terminal-tabs";
import type { TerminalSessionTabState } from "../model/types";

export type TerminalBatchCloseRequest = {
  labels: string[];
  sessionIds: string[];
};

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
  const [batchCloseRequest, setBatchCloseRequest] = useState<TerminalBatchCloseRequest | null>(
    null,
  );

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

  // Close a session without the confirmation prompt — used after the user
  // confirms a batch close, and by Reconnect which should stay one-click.
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

  function requestBatchClose(kind: TerminalBatchCloseKind, sessionId?: string) {
    const tabs = terminalBatchCloseTabs(sessionTabs, kind, sessionId);
    if (tabs.length === 0) {
      return;
    }
    setBatchCloseRequest({
      labels: tabs.map((item) => item.title),
      sessionIds: tabs.map((item) => item.session.sessionId),
    });
  }

  function closeOtherSessions(sessionId: string) {
    requestBatchClose("others", sessionId);
  }

  function closeAllSessions() {
    requestBatchClose("all");
  }

  function closeSessionsToLeft(sessionId: string) {
    requestBatchClose("left", sessionId);
  }

  function closeSessionsToRight(sessionId: string) {
    requestBatchClose("right", sessionId);
  }

  function confirmBatchClose() {
    if (!batchCloseRequest) {
      return;
    }
    const sessionIds = batchCloseRequest.sessionIds;
    setBatchCloseRequest(null);
    sessionIds.forEach((sessionId) => closeSessionNow(sessionId));
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
    batchCloseRequest,
    closeAllSessions,
    closeConfirmSession,
    closeConfirmSessionId,
    closeOtherSessions,
    closeSessionsToLeft,
    closeSessionsToRight,
    confirmBatchClose,
    confirmCloseSession,
    reconnectSession,
    requestCloseSession,
    setBatchCloseRequest,
    setCloseConfirmSessionId,
  };
}
