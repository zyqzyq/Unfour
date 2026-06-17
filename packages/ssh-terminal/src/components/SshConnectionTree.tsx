import {
  Copy,
  ExternalLink,
  MoreHorizontal,
  Plug,
  TerminalSquare,
} from "lucide-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  closeSshSession,
  connectSshSession,
  type SshConnection,
  type SshSessionSummary,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
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
  type TreeViewItem,
} from "@unfour/ui";
import { useSshConnections } from "../hooks/useSshConnections";
import { useTerminalSessions } from "../hooks/useTerminalSessions";
import { formatTerminalError } from "../model/errors";
import { useTerminalStore } from "../model/terminal-state";
import {
  terminalSessionStatus,
  terminalSessionStatusLabel,
} from "../model/terminal-session-status";

export function SshConnectionTree({
  active,
  collapsed,
  onEditConnection,
  onOpenTerminal,
  onOpenTerminalSplit,
  workspaceId,
}: {
  active?: boolean;
  collapsed?: boolean;
  onEditConnection?: (connection: SshConnection) => void;
  onOpenTerminal?: () => void;
  onOpenTerminalSplit?: (connection: SshConnection) => void;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();
  const { selectedSshConnectionId, setSelectedSshConnection } = useWorkspaceStore();
  const activeSessionId = useTerminalStore((state) => state.activeSessionId);
  const setActiveSessionId = useTerminalStore((state) => state.setActiveSessionId);
  const appendTerminalEvents = useTerminalStore((state) => state.appendTerminalEvents);
  const setSplitMode = useTerminalStore((state) => state.setSplitMode);
  const startTerminalSession = useTerminalStore((state) => state.startTerminalSession);
  const connectionsQuery = useSshConnections(workspaceId);
  const sessionsQuery = useTerminalSessions(workspaceId);
  const connections = connectionsQuery.data ?? [];
  const sessions = sessionsQuery.data ?? [];

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
          data: `Connected to ${session.username}@${session.host}. PTY ${session.cols}x${session.rows} allocated.\r\n`,
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
  });
  const closeMutation = useMutation({
    mutationFn: (sessionId: string) => closeSshSession({ workspaceId, sessionId }),
    onSuccess: (session) => {
      appendTerminalEvents([
        {
          sessionId: session.sessionId,
          kind: "close",
          data: "SSH session closed.\r\n",
          createdAt: session.updatedAt,
        },
      ]);
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });

  function connect(connection: SshConnection, split = false) {
    setSelectedSshConnection(connection.id);
    connectMutation.mutate({ connectionId: connection.id, split });
  }

  function disconnect(session: SshSessionSummary) {
    if (!["disconnected", "failed"].includes(session.status)) {
      const confirmed = window.confirm(`Disconnect SSH session ${session.username}@${session.host}?`);
      if (!confirmed) {
        return;
      }
    }
    closeMutation.mutate(session.sessionId);
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
          <span className="sr-only">SSH Sessions</span>
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
      ? "failed"
      : connecting
        ? "connecting"
        : activeSession
          ? terminalSessionStatusLabel(activeSession)
          : "disconnected";
    const menu = (
      <>
        <ContextMenuItem onSelect={() => connect(connection)}>Connect</ContextMenuItem>
        <ContextMenuItem
          disabled={!activeSession || closeMutation.isPending}
          onSelect={() => activeSession && disconnect(activeSession)}
        >
          Disconnect
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => connect(connection)}>Reconnect</ContextMenuItem>
        <ContextMenuItem onSelect={() => select(connection)}>Open in New Tab</ContextMenuItem>
        <ContextMenuItem
          onSelect={() => {
            setSelectedSshConnection(connection.id);
            connect(connection, true);
            onOpenTerminalSplit?.(connection);
          }}
        >
          Open in Split Pane
        </ContextMenuItem>
        <ContextMenuItem disabled={!onEditConnection} onSelect={() => onEditConnection?.(connection)}>
          Edit Connection
        </ContextMenuItem>
        <ContextMenuItem disabled>Duplicate Connection</ContextMenuItem>
        <ContextMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.host)}>
          Copy Host
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() =>
            void navigator.clipboard?.writeText(
              `ssh ${connection.username}@${connection.host} -p ${connection.port}`,
            )
          }
        >
          Copy SSH Command
        </ContextMenuItem>
        <ContextMenuItem disabled>Move to Group</ContextMenuItem>
        <ContextMenuItem disabled>Delete Connection</ContextMenuItem>
      </>
    );

    return {
      actions: (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <IconButton label={`SSH actions for ${connection.name}`}>
              <MoreHorizontal size={13} />
            </IconButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onSelect={() => connect(connection)}>
              <Plug size={13} />
              Connect
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => select(connection)}>
              <ExternalLink size={13} />
              Open
            </DropdownMenuItem>
            <DropdownMenuItem disabled={!onEditConnection} onSelect={() => onEditConnection?.(connection)}>
              Edit Connection
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.host)}>
              <Copy size={13} />
              Copy Host
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
      contextMenu: menu,
      icon: <TerminalSquare size={13} />,
      id: connection.id,
      label: connection.name,
      meta: <ConnectionStatus label={connectionStatusLabel} status={connectionStatus} />,
      title: `${connection.name} ${connection.username}@${connection.host}`,
    };
  });

  const items: TreeViewItem[] = [
    {
      children: connectionItems,
      id: "ssh-connections",
      label: "SSH Connections",
      meta: <StatusBadge>{connections.length}</StatusBadge>,
    },
    {
      children: sessions.map((session) => ({
        icon: <TerminalSquare size={13} />,
        id: `session:${session.sessionId}`,
        label: `${session.username}@${session.host}`,
        meta: (
          <ConnectionStatus
            label={terminalSessionStatusLabel(session)}
            status={terminalSessionStatus(session)}
          />
        ),
        title: `${session.username}@${session.host} ${session.cols}x${session.rows}`,
      })),
      id: "ssh-sessions",
      label: "Sessions",
      meta: <StatusBadge>{sessions.length}</StatusBadge>,
    },
  ];

  return (
    <SidebarSection title="SSH">
      <SidebarRow active={active && !selectedSshConnectionId} onClick={onOpenTerminal}>
        <TerminalSquare size={14} />
        <span className="min-w-0 flex-1 truncate">SSH Sessions</span>
        <StatusBadge>{sessions.length}</StatusBadge>
      </SidebarRow>
      {connections.length ? (
        <TreeView
          defaultExpandedIds={["ssh-connections", "ssh-sessions"]}
          items={items}
          onSelect={(item) => {
            const connection = connections.find((candidate) => candidate.id === item.id);
            if (connection) {
              select(connection);
              return;
            }

            if (item.id.startsWith("session:")) {
              setActiveSessionId(item.id.slice("session:".length));
              onOpenTerminal?.();
            }
          }}
          selectedId={activeSessionId ? `session:${activeSessionId}` : selectedSshConnectionId}
        />
      ) : (
        <EmptyState className="min-h-[72px]">No SSH connections</EmptyState>
      )}
      {connectionsQuery.error && (
        <StatusBadge tone="danger">Connections failed to load</StatusBadge>
      )}
      {connectMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(connectMutation.error)}
        </StatusBadge>
      )}
      {closeMutation.error && (
        <StatusBadge className="max-w-full" tone="danger">
          {formatTerminalError(closeMutation.error)}
        </StatusBadge>
      )}
    </SidebarSection>
  );
}
