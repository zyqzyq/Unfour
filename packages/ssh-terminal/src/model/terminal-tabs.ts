import type { SshConnection, SshSessionSummary } from "@unfour/command-client";
import type { TerminalSessionTabState } from "./types";

export function shouldCloseTerminalSessionInBackend({
  frontendFailedSessions,
  sessionId,
}: {
  frontendFailedSessions: Readonly<Record<string, SshSessionSummary>>;
  sessionId: string;
}) {
  return frontendFailedSessions[sessionId] === undefined;
}

export function shouldShowTerminalSessionTab({
  activeSessionId,
  dismissedSessionIds,
  session,
}: {
  activeSessionId: string | null;
  dismissedSessionIds: string[];
  session: SshSessionSummary;
}) {
  if (dismissedSessionIds.includes(session.sessionId)) {
    return false;
  }

  if (session.status === "disconnected" && session.sessionId !== activeSessionId) {
    return false;
  }

  return true;
}

export type TerminalBatchCloseKind = "all" | "left" | "others" | "right";

export function terminalBatchCloseTabs(
  sessionTabs: TerminalSessionTabState[],
  kind: TerminalBatchCloseKind,
  sessionId?: string,
): TerminalSessionTabState[] {
  switch (kind) {
    case "all":
      return [...sessionTabs];
    case "others":
      return sessionTabs.filter((item) => item.session.sessionId !== sessionId);
    case "left": {
      const index = sessionTabs.findIndex((item) => item.session.sessionId === sessionId);
      return index > 0 ? sessionTabs.slice(0, index) : [];
    }
    case "right": {
      const index = sessionTabs.findIndex((item) => item.session.sessionId === sessionId);
      return index >= 0 ? sessionTabs.slice(index + 1) : [];
    }
  }
}

export function buildTerminalSessionTabs({
  connections,
  sessions,
}: {
  connections: SshConnection[];
  sessions: SshSessionSummary[];
}): TerminalSessionTabState[] {
  const titleCounts = new Map<string, number>();

  return sessions.map((session) => {
    const connection = connections.find((item) => item.id === session.connectionId) ?? null;
    const baseTitle = connection?.name ?? `${session.username}@${session.host}`;
    const count = titleCounts.get(baseTitle) ?? 0;
    titleCounts.set(baseTitle, count + 1);

    return {
      connection,
      session,
      title: count === 0 ? baseTitle : `${baseTitle} ${count + 1}`,
    };
  });
}
