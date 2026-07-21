import { useState } from "react";
import {
  Copy,
  CopyPlus,
  ExternalLink,
  MoreHorizontal,
  Pencil,
  Plug,
  Plus,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  closeSshSession,
  connectSshSession,
  deleteSshConnection,
  saveSshConnection,
  type SshConnection,
  type SshSessionSummary,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
  ConfirmDialog,
  ConnectionStatus,
  ContextMenuItem,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  IconButton,
  SidebarRow,
  SidebarSection,
  StatusBadge,
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import { useSshConnections } from "../hooks/useSshConnections";
import { useTerminalSessions } from "../hooks/useTerminalSessions";
import { formatTerminalError } from "../model/errors";
import { sshConnectionToInput } from "../model/ssh-connection-state";
import { useTerminalStore } from "../model/terminal-state";
import {
  terminalSessionStatus,
  terminalSessionStatusLabel,
} from "../model/terminal-session-status";
import { SshSidebarModeSwitcher } from "./SshSidebarModeSwitcher";

export function SshConnectionTree({
  active,
  collapsed,
  onEditConnection,
  onNewConnection,
  onOpenTasks,
  onOpenTerminal,
  workspaceId,
}: {
  active?: boolean;
  collapsed?: boolean;
  onEditConnection?: (connection: SshConnection) => void;
  onNewConnection?: () => void;
  onOpenTasks?: () => void;
  onOpenTerminal?: () => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { selectedSshConnectionId, setSelectedSshConnection } = useWorkspaceStore();
  const appendTerminalEvents = useTerminalStore((state) => state.appendTerminalEvents);
  const addFrontendFailedSession = useTerminalStore(
    (state) => state.addFrontendFailedSession,
  );
  const setSplitMode = useTerminalStore((state) => state.setSplitMode);
  const startTerminalSession = useTerminalStore((state) => state.startTerminalSession);
  const connectionsQuery = useSshConnections(workspaceId);
  const sessionsQuery = useTerminalSessions(workspaceId);
  const connections = connectionsQuery.data ?? [];
  const sessions = sessionsQuery.data ?? [];
  const [confirm, setConfirm] = useState<
    | { kind: "disconnect"; session: SshSessionSummary }
    | { kind: "delete"; connection: SshConnection }
    | null
  >(null);

  const connectMutation = useMutation({
    mutationFn: ({
      connectionId,
    }: {
      connectionId: string;
      split?: boolean;
    }) => connectSshSession({ workspaceId, connectionId, cols: 120, rows: 32 }),
    onSuccess: (session, variables) => {
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
      if (variables.split) {
        setSplitMode("vertical");
      }
      queryClient.setQueryData<SshSessionSummary[]>(
        ["ssh-sessions", workspaceId],
        (current = []) => [
          ...current.filter((item) => item.sessionId !== session.sessionId),
          session,
        ],
      );
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
      onOpenTerminal?.();
    },
    onError: (error, variables) => {
      const connection = connections.find((c) => c.id === variables.connectionId);
      if (!connection) return;
      const syntheticId = `__frontend_failed_${variables.connectionId}_${Date.now()}`;
      const now = new Date().toISOString();
      const failedSession: SshSessionSummary = {
        sessionId: syntheticId,
        workspaceId,
        connectionId: variables.connectionId,
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
      onOpenTerminal?.();
    },
  });
  const closeMutation = useMutation({
    mutationFn: (sessionId: string) => closeSshSession({ workspaceId, sessionId }),
    onSuccess: (session) => {
      appendTerminalEvents([
        {
          sessionId: session.sessionId,
          kind: "close",
          data: `${t("ssh.session.closed")}\r\n`,
          createdAt: session.updatedAt,
        },
      ]);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteSshConnection(workspaceId, connectionId),
    onSuccess: (_result, connectionId) => {
      if (selectedSshConnectionId === connectionId) {
        setSelectedSshConnection(null);
      }
      setConfirm(null);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });
  // Clone a connection into a new record. The saved credential is shared by
  // reusing its reference (the plaintext secret is never exposed to the client),
  // so the copy can connect immediately without re-entering the password.
  const duplicateMutation = useMutation({
    mutationFn: (connection: SshConnection) =>
      saveSshConnection({
        ...sshConnectionToInput(connection, workspaceId),
        id: undefined,
        name: t("ssh.tree.copyName", { name: connection.name }),
      }),
    onSuccess: (created) => {
      setSelectedSshConnection(created.id);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
    },
  });

  function connect(connection: SshConnection, split = false) {
    connectMutation.reset();
    setSelectedSshConnection(connection.id);
    connectMutation.mutate({ connectionId: connection.id, split });
  }

  function disconnect(session: SshSessionSummary) {
    if (!["disconnected", "failed"].includes(session.status)) {
      setConfirm({ kind: "disconnect", session });
      return;
    }
    closeMutation.mutate(session.sessionId);
  }

  function confirmAction() {
    if (!confirm) {
      return;
    }
    if (confirm.kind === "disconnect") {
      closeMutation.mutate(confirm.session.sessionId);
      setConfirm(null);
      return;
    }
    deleteMutation.mutate(confirm.connection.id);
  }

  function select(connection: SshConnection) {
    setSelectedSshConnection(connection.id);
    onOpenTerminal?.();
  }

  if (collapsed) {
    return (
      <SidebarSection>
        <SidebarRow active={active} onClick={onOpenTerminal}>
          <TerminalSquare size={14} />
          <span className="sr-only">{t("ssh.status.sshSessions")}</span>
        </SidebarRow>
      </SidebarSection>
    );
  }

  const activeSessionByConnection = new Map<string, SshSessionSummary>();
  sessions
    .filter((session) =>
      ["connected", "degraded", "reconnecting"].includes(session.status),
    )
    .forEach((session) => {
      if (!activeSessionByConnection.has(session.connectionId)) {
        activeSessionByConnection.set(session.connectionId, session);
      }
    });

  const connectionItems: TreeViewItem[] = connections.map((connection) => {
    const activeSession = activeSessionByConnection.get(connection.id);
    const connecting =
      connectMutation.isPending &&
      connectMutation.variables?.connectionId === connection.id;
    const connectionFailed =
      Boolean(connectMutation.error) &&
      connectMutation.variables?.connectionId === connection.id;
    const connectionStatus = connectionFailed
      ? "error"
      : connecting
        ? "connecting"
        : activeSession
          ? terminalSessionStatus(activeSession)
          : "disconnected";
    const connectionStatusLabel = connectionFailed
      ? t("ssh.sessionStatus.failed")
      : connecting
        ? t("ssh.sessionStatus.connecting")
        : activeSession
          ? terminalSessionStatusLabel(activeSession, t)
          : t("ssh.sessionStatus.disconnected");
    const menu = (
      <>
        <ContextMenuItem onSelect={() => connect(connection)}>{t("ssh.tree.connect")}</ContextMenuItem>
        <ContextMenuItem
          disabled={!activeSession || closeMutation.isPending}
          onSelect={() => activeSession && disconnect(activeSession)}
        >
          {t("ssh.tree.disconnect")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => select(connection)}>{t("ssh.tree.openInNewTab")}</ContextMenuItem>
        <ContextMenuItem
          onSelect={() => {
            setSelectedSshConnection(connection.id);
            connect(connection, true);
          }}
        >
          {t("ssh.tree.openInSplit")}
        </ContextMenuItem>
        {onEditConnection && (
          <ContextMenuItem onSelect={() => onEditConnection(connection)}>
            {t("ssh.tree.editConnection")}
          </ContextMenuItem>
        )}
        <ContextMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.host)}>
          {t("ssh.tree.copyHost")}
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() =>
            void navigator.clipboard?.writeText(
              `ssh ${connection.username}@${connection.host} -p ${connection.port}`,
            )
          }
        >
          {t("ssh.tree.copySshCommand")}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => setConfirm({ kind: "delete", connection })} tone="danger">
          {t("ssh.tree.deleteConnection")}
        </ContextMenuItem>
      </>
    );

    return {
      actions: (
        <span className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          <IconButton
            label={t("ssh.tree.connect")}
            onClick={() => connect(connection)}
            size="compact"
          >
            <Plug size={13} />
          </IconButton>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <IconButton
                label={t("ssh.tree.actionsLabel", { name: connection.name })}
                size="compact"
              >
                <MoreHorizontal size={13} />
              </IconButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              <DropdownMenuItem onSelect={() => connect(connection)}>
                <Plug size={13} />
                {t("ssh.tree.connect")}
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => select(connection)}>
                <ExternalLink size={13} />
                {t("ssh.tree.open")}
              </DropdownMenuItem>
              {onEditConnection && (
                <DropdownMenuItem onSelect={() => onEditConnection(connection)}>
                  <Pencil size={13} />
                  {t("ssh.tree.editConnection")}
                </DropdownMenuItem>
              )}
              <DropdownMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.host)}>
                <Copy size={13} />
                {t("ssh.tree.copyHost")}
              </DropdownMenuItem>
              <DropdownMenuItem
                disabled={duplicateMutation.isPending}
                onSelect={() => duplicateMutation.mutate(connection)}
              >
                <CopyPlus size={13} />
                {t("ssh.tree.duplicateConnection")}
              </DropdownMenuItem>
              <DropdownMenuItem
                className="text-[var(--u-color-danger)]"
                onSelect={() => setConfirm({ kind: "delete", connection })}
              >
                <Trash2 size={13} />
                {t("ssh.tree.deleteConnection")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </span>
      ),
      contextMenu: menu,
      icon: <TerminalSquare size={13} />,
      id: connection.id,
      label: <span className="truncate">{connection.name}</span>,
      meta: (
        <ConnectionStatus dotOnly label={connectionStatusLabel} status={connectionStatus} variant="dot" />
      ),
      title: `${connection.name} ${connection.username}@${connection.host}`,
    };
  });

  return (
    <SidebarSection>
      <div className="flex h-7 items-center justify-between gap-2 px-1">
        <SshSidebarModeSwitcher
          activeMode="connections"
          onChange={(mode) => mode === "tasks" && onOpenTasks?.()}
        />
        {onNewConnection && (
          <IconButton
            label={t("ssh.actions.newConnection")}
            onClick={onNewConnection}
            size="compact"
          >
            <Plus size={14} />
          </IconButton>
        )}
      </div>
      {connections.length ? (
        <TreeView
          items={connectionItems}
          onActivate={(item) => {
            const connection = connections.find((candidate) => candidate.id === item.id);
            if (connection) {
              connect(connection);
            }
          }}
          onSelect={(item) => {
            const connection = connections.find((candidate) => candidate.id === item.id);
            if (connection) {
              select(connection);
            }
          }}
          selectedId={selectedSshConnectionId}
        />
      ) : (
        <EmptyState className="min-h-[72px]">{t("ssh.empty.noConnectionsShort")}</EmptyState>
      )}
      {connectionsQuery.error && (
        <StatusBadge tone="danger">{t("ssh.empty.connectionsFailed")}</StatusBadge>
      )}
      {connectMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(connectMutation.error, t)}
        </StatusBadge>
      )}
      {closeMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(closeMutation.error, t)}
        </StatusBadge>
      )}
      {deleteMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(deleteMutation.error, t)}
        </StatusBadge>
      )}
      {duplicateMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(duplicateMutation.error, t)}
        </StatusBadge>
      )}
      <ConfirmDialog
        confirmLabel={
          confirm?.kind === "delete"
            ? t("common.actions.delete")
            : t("common.actions.disconnect")
        }
        description={
          confirm?.kind === "delete"
            ? t("ssh.tree.deleteBody", { name: confirm.connection.name })
            : confirm
              ? t("ssh.session.disconnectConfirm", {
                  label: `${confirm.session.username}@${confirm.session.host}`,
                })
              : ""
        }
        onConfirm={confirmAction}
        onOpenChange={(open) => !open && setConfirm(null)}
        open={confirm !== null}
        pending={confirm?.kind === "delete" ? deleteMutation.isPending : closeMutation.isPending}
        title={
          confirm?.kind === "delete"
            ? t("ssh.tree.deleteTitle")
            : t("ssh.session.disconnectTitle")
        }
      />
    </SidebarSection>
  );
}
