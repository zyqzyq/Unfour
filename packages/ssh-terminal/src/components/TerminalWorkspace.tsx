import { FilePlus2, TerminalSquare } from "lucide-react";
import type {
  SshConnection,
  SshSessionEvent,
  SshSessionSummary,
} from "@unfour/command-client";
import {
  Button,
  ConnectionStatus,
  EmptyState,
  ErrorState,
  StatusBadge,
  Tabs,
} from "@unfour/ui";
import type { TerminalSplitMode, TerminalSessionTabState } from "../model/types";
import { formatTerminalError } from "../model/errors";
import { sshEndpointLabel } from "../model/ssh-connection-state";
import {
  terminalSessionStatus,
  terminalSessionStatusLabel,
} from "../model/terminal-session-status";
import { TerminalSearchBar } from "./TerminalSearchBar";
import { TerminalSessionTabMeta } from "./TerminalSessionTab";
import { TerminalSplitView } from "./TerminalSplitView";

export function TerminalWorkspace({
  activeSession,
  activeSessionId,
  actionError,
  canStartSession,
  emptyMessage,
  error,
  events,
  onNewConnection,
  onNewSession,
  onCloseSession,
  onSelectSession,
  selectedConnection,
  selectedConnectionStatus,
  sessions,
  splitMode,
}: {
  activeSession: SshSessionSummary | null;
  activeSessionId: string | null;
  actionError?: unknown;
  canStartSession: boolean;
  emptyMessage: string;
  error?: unknown;
  events: SshSessionEvent[];
  onNewConnection: () => void;
  onNewSession: () => void;
  onCloseSession: (sessionId: string) => void;
  onSelectSession: (sessionId: string) => void;
  selectedConnection: SshConnection | null;
  selectedConnectionStatus: "connecting" | SshSessionSummary["status"] | "disconnected";
  sessions: TerminalSessionTabState[];
  splitMode: TerminalSplitMode;
}) {
  const hasSessions = sessions.length > 0;
  const secondarySession =
    sessions.find(
      (item) =>
        item.session.sessionId !== activeSessionId && item.session.status === "connected",
    )?.session ??
    sessions.find((item) => item.session.sessionId !== activeSessionId)?.session ??
    null;

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <TerminalWorkspaceHeader
        activeSession={activeSession}
        actionError={actionError}
        selectedConnection={selectedConnection}
        selectedConnectionStatus={selectedConnectionStatus}
      />
      {hasSessions ? (
        <Tabs
          activeId={activeSessionId ?? sessions[0]?.session.sessionId ?? ""}
          onClose={onCloseSession}
          onSelect={onSelectSession}
          tabs={sessions.map((item) => ({
            id: item.session.sessionId,
            loading:
              item.session.status === "connected" &&
              item.session.sessionId !== activeSessionId,
            meta: <TerminalSessionTabMeta session={item.session} />,
            modified: item.modified,
            title: item.title,
          }))}
        />
      ) : null}
      <div className="relative flex min-h-0 flex-1">
        <TerminalSearchBar />
        {error ? (
          <ErrorState className="h-full min-h-0 flex-1 rounded-none border-0">
            {formatTerminalError(error)}
          </ErrorState>
        ) : hasSessions || activeSession ? (
          <TerminalSplitView
            activeSession={activeSession}
            activeEvents={events.filter(
              (event) => event.sessionId === activeSession?.sessionId,
            )}
            secondaryEvents={events.filter(
              (event) => event.sessionId === secondarySession?.sessionId,
            )}
            secondarySession={secondarySession}
            splitMode={splitMode}
          />
        ) : (
          <EmptyState className="h-full min-h-0 flex-1 rounded-none border-0">
            <div className="flex max-w-[520px] flex-col items-center gap-3">
              <div className="space-y-1">
                <div className="text-[13px] font-semibold text-[var(--u-color-text)]">
                  No SSH session is open
                </div>
                <div>{emptyMessage}</div>
              </div>
              <div className="flex flex-wrap justify-center gap-2">
                <Button onClick={onNewConnection} size="sm" type="button" variant="outline">
                  <FilePlus2 size={14} />
                  New Connection
                </Button>
                <Button
                  disabled={!canStartSession}
                  onClick={onNewSession}
                  size="sm"
                  type="button"
                >
                  <TerminalSquare size={14} />
                  Open Session
                </Button>
              </div>
            </div>
          </EmptyState>
        )}
      </div>
    </div>
  );
}

function TerminalWorkspaceHeader({
  activeSession,
  actionError,
  selectedConnection,
  selectedConnectionStatus,
}: {
  activeSession: SshSessionSummary | null;
  actionError?: unknown;
  selectedConnection: SshConnection | null;
  selectedConnectionStatus: "connecting" | SshSessionSummary["status"] | "disconnected";
}) {
  const sessionLabel = activeSession
    ? `${activeSession.username}@${activeSession.host}`
    : null;
  const endpoint = selectedConnection
    ? sshEndpointLabel(selectedConnection)
    : sessionLabel ?? "No connection selected";
  const status =
    selectedConnectionStatus === "connecting"
      ? "connecting"
      : activeSession
        ? terminalSessionStatus(activeSession)
        : selectedConnectionStatus === "failed"
          ? "error"
          : selectedConnectionStatus === "degraded" ||
              selectedConnectionStatus === "reconnecting"
            ? "connecting"
            : selectedConnectionStatus;
  const statusLabel =
    selectedConnectionStatus === "connecting"
      ? "connecting"
      : activeSession
        ? terminalSessionStatusLabel(activeSession)
        : selectedConnectionStatus;

  return (
    <div className="flex min-h-[34px] shrink-0 items-center gap-3 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 text-[12px]">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span className="min-w-0 truncate font-semibold text-[var(--u-color-text)]">
          {selectedConnection?.name ?? sessionLabel ?? "SSH Terminal"}
        </span>
        <span className="min-w-0 truncate text-[var(--u-color-text-muted)]">
          {endpoint}
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <ConnectionStatus
          label={statusLabel}
          status={status}
        />
        {activeSession && (
          <StatusBadge>
            {activeSession.cols}x{activeSession.rows}
          </StatusBadge>
        )}
        <StatusBadge>{selectedConnection?.authKind ?? activeSession?.authKind ?? "no auth"}</StatusBadge>
      </div>
      {Boolean(actionError) && (
        <div className="min-w-0 max-w-[38%] truncate text-[var(--u-color-danger)]">
          {formatTerminalError(actionError)}
        </div>
      )}
    </div>
  );
}
