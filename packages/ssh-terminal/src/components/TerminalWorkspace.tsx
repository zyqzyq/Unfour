import {
  ChevronsLeft,
  ChevronsRight,
  CircleX,
  CopyPlus,
  Eraser,
  FilePlus2,
  Loader2,
  PanelRightClose,
  Pencil,
  Plug,
  RefreshCw,
  TerminalSquare,
  Unplug,
  X,
} from "lucide-react";
import type {
  SshConnection,
  SshSessionEvent,
  SshSessionSummary,
} from "@unfour/command-client";
import {
  Button,
  ContextMenuItem,
  ContextMenuSeparator,
  EmptyState,
  ErrorState,
  Tabs,
  useI18n,
} from "@unfour/ui";
import type { TerminalSplitMode, TerminalSessionTabState } from "../model/types";
import { formatTerminalError } from "../model/errors";
import { sshEndpointLabel } from "../model/ssh-connection-state";
import { TerminalSearchBar } from "./TerminalSearchBar";
import { TerminalSessionTabMeta } from "./TerminalSessionTab";
import { TerminalSplitView, type TerminalPaneModel } from "./TerminalSplitView";
import { useTerminalSplit } from "../hooks/useTerminalSplit";

export function TerminalWorkspace({
  activeSession,
  activeSessionId,
  actionError,
  connecting,
  emptyMessage,
  error,
  events,
  onClear,
  onCloseAll,
  onCloseLeft,
  onCloseOthers,
  onCloseRight,
  onCloseSession,
  onDuplicate,
  onNewConnection,
  onNewSession,
  onOpenPreferences,
  onReconnect,
  onRetry,
  onSelectSession,
  selectedConnection,
  sessions,
  splitMode,
}: {
  activeSession: SshSessionSummary | null;
  activeSessionId: string | null;
  actionError?: unknown;
  connecting?: boolean;
  emptyMessage: string;
  error?: unknown;
  events: SshSessionEvent[];
  onClear: (sessionId: string) => void;
  onCloseAll: () => void;
  onCloseLeft: (sessionId: string) => void;
  onCloseOthers: (sessionId: string) => void;
  onCloseRight: (sessionId: string) => void;
  onCloseSession: (sessionId: string) => void;
  onDuplicate: (connectionId: string) => void;
  onNewConnection: () => void;
  onNewSession: () => void;
  onOpenPreferences: (connection?: SshConnection | null) => void;
  onReconnect: (sessionId: string) => void;
  onRetry: (connectionId: string) => void;
  onSelectSession: (sessionId: string) => void;
  selectedConnection: SshConnection | null;
  sessions: TerminalSessionTabState[];
  splitMode: TerminalSplitMode;
}) {
  const { t } = useI18n();
  const { setMode } = useTerminalSplit();
  const hasSessions = sessions.length > 0;
  const secondaryTab =
    sessions.find(
      (item) =>
        item.session.sessionId !== activeSessionId && item.session.status === "connected",
    ) ??
    sessions.find((item) => item.session.sessionId !== activeSessionId) ??
    null;
  const activeTab =
    sessions.find((item) => item.session.sessionId === activeSession?.sessionId) ?? null;

  const primaryModel: TerminalPaneModel | null = activeSession
    ? {
        connection: activeTab?.connection ?? selectedConnection,
        events: events.filter((event) => event.sessionId === activeSession.sessionId),
        session: activeSession,
      }
    : null;
  const secondaryModel: TerminalPaneModel | null = secondaryTab
    ? {
        connection: secondaryTab.connection,
        events: events.filter(
          (event) => event.sessionId === secondaryTab.session.sessionId,
        ),
        session: secondaryTab.session,
      }
    : null;

  function tabContextMenu(item: TerminalSessionTabState) {
    const { connectionId, sessionId } = item.session;
    const index = sessions.findIndex((s) => s.session.sessionId === sessionId);
    const hasOthers = sessions.length > 1;
    const isFirst = index <= 0;
    const isLast = index === sessions.length - 1;
    return (
      <>
        <ContextMenuItem onSelect={() => onCloseSession(sessionId)} shortcut="Ctrl+W">
          <Unplug size={13} />
          {t("ssh.actions.closeConnection")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => onReconnect(sessionId)}>
          <RefreshCw size={13} />
          {t("ssh.actions.reconnectSession")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem disabled={!hasOthers} onSelect={() => onCloseOthers(sessionId)}>
          <X size={13} />
          {t("ssh.actions.closeOtherSessions")}
        </ContextMenuItem>
        <ContextMenuItem disabled={isFirst} onSelect={() => onCloseLeft(sessionId)}>
          <ChevronsLeft size={13} />
          {t("ssh.actions.closeSessionsToLeft")}
        </ContextMenuItem>
        <ContextMenuItem disabled={isLast} onSelect={() => onCloseRight(sessionId)}>
          <ChevronsRight size={13} />
          {t("ssh.actions.closeSessionsToRight")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => onCloseAll()} tone="danger">
          <CircleX size={13} />
          {t("ssh.actions.closeAllSessions")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => onDuplicate(connectionId)}>
          <CopyPlus size={13} />
          {t("ssh.actions.duplicateSession")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => onClear(sessionId)} shortcut="Ctrl+L">
          <Eraser size={13} />
          {t("ssh.actions.clearTerminal")}
        </ContextMenuItem>
      </>
    );
  }

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      {hasSessions ? (
        <Tabs
          activeId={activeSessionId ?? sessions[0]?.session.sessionId ?? ""}
          onClose={onCloseSession}
          onSelect={onSelectSession}
          tabs={sessions.map((item) => ({
            contextMenu: tabContextMenu(item),
            id: item.session.sessionId,
            meta: <TerminalSessionTabMeta session={item.session} />,
            modified: item.modified,
            title: `${item.session.username}@${item.session.host}`,
          }))}
        />
      ) : null}
      {Boolean(actionError) && (
        <div className="shrink-0 truncate border-b border-[var(--u-color-border)] bg-[var(--u-color-danger-soft)] px-3 py-1 text-[12px] text-[var(--u-color-danger)]">
          {formatTerminalError(actionError)}
        </div>
      )}
      <div className="relative flex min-h-0 flex-1">
        {splitMode !== "single" && (
          <Button
            className="absolute left-3 top-3 z-10"
            onClick={() => setMode("single")}
            size="sm"
            type="button"
            variant="outline"
          >
            <PanelRightClose size={14} />
            {t("ssh.actions.singlePane")}
          </Button>
        )}
        <TerminalSearchBar />
        {error ? (
          <ErrorState className="h-full min-h-0 flex-1 rounded-none border-0">
            {formatTerminalError(error)}
          </ErrorState>
        ) : primaryModel ? (
          <TerminalSplitView
            onRetry={onRetry}
            primary={primaryModel}
            secondary={secondaryModel}
            splitMode={splitMode}
          />
        ) : selectedConnection ? (
          <ReadyToConnectState
            connecting={connecting}
            connection={selectedConnection}
            onEditConnection={() => onOpenPreferences()}
            onNewSession={onNewSession}
          />
        ) : (
          <EmptyState className="h-full min-h-0 flex-1 rounded-none border-0">
            <div className="flex max-w-[520px] flex-col items-center gap-3">
              <div className="space-y-1">
                <div className="text-[13px] font-semibold text-[var(--u-color-text)]">
                  {t("ssh.empty.noSessionOpen")}
                </div>
                <div>{emptyMessage}</div>
              </div>
              <Button onClick={onNewConnection} size="sm" type="button" variant="outline">
                <FilePlus2 size={14} />
                {t("ssh.actions.newConnection")}
              </Button>
            </div>
          </EmptyState>
        )}
      </div>
    </div>
  );
}

function ReadyToConnectState({
  connecting,
  connection,
  onEditConnection,
  onNewSession,
}: {
  connecting?: boolean;
  connection: SshConnection;
  onEditConnection: () => void;
  onNewSession: () => void;
}) {
  const { t } = useI18n();
  return (
    <EmptyState className="h-full min-h-0 flex-1 rounded-none border-0">
      <div className="flex max-w-[520px] flex-col items-center gap-3">
        <div className="grid h-[52px] w-[52px] place-items-center rounded-[var(--u-radius-lg)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]">
          {connecting ? (
            <Loader2 className="animate-spin" size={24} />
          ) : (
            <Plug size={24} />
          )}
        </div>
        <div className="space-y-1">
          <div className="text-[14px] font-semibold text-[var(--u-color-text)]">
            {connecting ? t("ssh.pane.connecting") : t("ssh.pane.readyTitle")}
          </div>
          <div>
            {connecting
              ? t("ssh.pane.connectingDetail", {
                  host: connection.host,
                  port: connection.port ?? 22,
                })
              : t("ssh.pane.readyDetail", {
                  endpoint: sshEndpointLabel(connection),
                  name: connection.name,
                })}
          </div>
        </div>
        <div className="flex flex-wrap justify-center gap-2">
          <Button
            disabled={connecting}
            onClick={onEditConnection}
            size="sm"
            type="button"
            variant="outline"
          >
            <Pencil size={14} />
            {t("ssh.pane.editConnection")}
          </Button>
          <Button disabled={connecting} onClick={onNewSession} size="sm" type="button">
            {connecting ? (
              <Loader2 className="animate-spin" size={14} />
            ) : (
              <TerminalSquare size={14} />
            )}
            {t("ssh.pane.openSession")}
          </Button>
        </div>
      </div>
    </EmptyState>
  );
}
