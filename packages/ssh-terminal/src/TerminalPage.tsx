import { FormEvent, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  cancelSshReconnect,
  closeSshSession,
  connectSshSession,
  deleteSshConnection,
  exportSshLog,
  getSshHostFingerprint,
  getSshSessionHistory,
  saveSshConnection,
  type SshConnection,
  type SshConnectionInput,
  type SshHostFingerprintInfo,
  type SshSessionSummary,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { ConfirmDialog, LoadingState, useI18n } from "@unfour/ui";
import { TerminalWorkspace } from "./components/TerminalWorkspace";
import { SshConnectionTree } from "./components/SshConnectionTree";
import { SshConnectionDialog } from "./components/SshConnectionDialog";
import { HostKeyTrustDialog } from "./components/HostKeyTrustDialog";
import { useSshConnections } from "./hooks/useSshConnections";
import { useTerminalSessions } from "./hooks/useTerminalSessions";
import { useTerminalSplit } from "./hooks/useTerminalSplit";
import {
  redactTerminalLog,
  useTerminalStore,
} from "./model/terminal-state";
import {
  defaultSshConnectionInput,
  sshConnectionToInput,
} from "./model/ssh-connection-state";
import { buildTerminalSessionTabs } from "./model/terminal-tabs";

export function TerminalPage({
  onShellSidebarChange,
  workspaceId,
}: {
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    selectedSshConnectionId: selectedConnectionId,
    setActiveTab,
    setSelectedSshConnection,
  } = useWorkspaceStore();
  const split = useTerminalSplit();
  const activeSessionId = useTerminalStore((state) => state.activeSessionId);
  const activateWorkspace = useTerminalStore((state) => state.activateWorkspace);
  const appendTerminalEvents = useTerminalStore((state) => state.appendTerminalEvents);
  const clearTerminalSessionEvents = useTerminalStore(
    (state) => state.clearTerminalSessionEvents,
  );
  const hydrateTerminalSession = useTerminalStore(
    (state) => state.hydrateTerminalSession,
  );
  const resetTerminalEvents = useTerminalStore((state) => state.resetTerminalEvents);
  const setActiveSessionId = useTerminalStore((state) => state.setActiveSessionId);
  const setExportedLog = useTerminalStore((state) => state.setExportedLog);
  const setSearchOpen = useTerminalStore((state) => state.setSearchOpen);
  const startTerminalSession = useTerminalStore((state) => state.startTerminalSession);
  const terminalEvents = useTerminalStore((state) => state.terminalEvents);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [closeConfirmSessionId, setCloseConfirmSessionId] = useState<string | null>(null);
  const [trustDialogState, setTrustDialogState] = useState<{
    open: boolean;
    connectionId: string | null;
    host: string;
    port: number;
    fingerprint: SshHostFingerprintInfo | null | undefined;
    mismatchError: string | null;
  }>({
    open: false,
    connectionId: null,
    host: "",
    port: 22,
    fingerprint: undefined,
    mismatchError: null,
  });
  const hydratedSessionIdsRef = useRef(new Set<string>());
  const [form, setForm] = useState<SshConnectionInput>(() =>
    defaultSshConnectionInput(workspaceId),
  );

  const connectionsQuery = useSshConnections(workspaceId);
  const sessionsQuery = useTerminalSessions(workspaceId);
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const sessions = useMemo(() => sessionsQuery.data ?? [], [sessionsQuery.data]);
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  const prevSelectedConnectionIdRef = useRef(selectedConnectionId);
  const activeSession = useMemo(
    () => sessions.find((item) => item.sessionId === activeSessionId) ?? null,
    [activeSessionId, sessions],
  );
  const sessionTabs = useMemo(
    () => buildTerminalSessionTabs({ connections, sessions }),
    [connections, sessions],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listen<{
      sessionId: string;
      data: string;
      status?: SshSessionSummary["status"] | null;
      reconnectAttempt?: number;
    }>("ssh://terminal-data", (event) => {
      const payload = event.payload;
      if (!payload?.sessionId) {
        return;
      }
      if (payload.data) {
        appendTerminalEvents([
          {
            sessionId: payload.sessionId,
            kind:
              payload.status === "disconnected" || payload.status === "failed"
                ? "close"
                : "output",
            data: payload.data,
            createdAt: new Date().toISOString(),
          },
        ]);
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
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
        } else {
          unlisten = dispose;
        }
      })
      .catch(() => {
        // Browser mock mode has no Tauri event transport; query polling remains active.
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [appendTerminalEvents, queryClient, workspaceId]);

  useEffect(() => {
    activateWorkspace(workspaceId);
    hydratedSessionIdsRef.current.clear();
  }, [activateWorkspace, workspaceId]);

  useEffect(() => {
    const pending = sessions.filter(
      (session) => !hydratedSessionIdsRef.current.has(session.sessionId),
    );
    pending.forEach((session) => hydratedSessionIdsRef.current.add(session.sessionId));
    pending.forEach((session) => {
      getSshSessionHistory({
        workspaceId,
        sessionId: session.sessionId,
      })
        .then((events) => hydrateTerminalSession(session.sessionId, events))
        .catch(() => {
          hydratedSessionIdsRef.current.delete(session.sessionId);
        });
    });
  }, [hydrateTerminalSession, sessions, workspaceId]);

  useEffect(() => {
    if (!connections.length) {
      if (selectedConnectionId) {
        setSelectedSshConnection(null);
      }
      return;
    }

    if (!selectedConnectionId || !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedSshConnection(connections[0].id);
    }
  }, [connections, selectedConnectionId, setSelectedSshConnection]);

  // Sync form state when the selected connection changes (render-time adjustment pattern).
  /* eslint-disable react-hooks/refs -- render-time ref read/write is React's recommended pattern for adjusting state when a derived value changes */
  if (selectedConnectionId !== prevSelectedConnectionIdRef.current) {
    prevSelectedConnectionIdRef.current = selectedConnectionId;
    /* eslint-enable react-hooks/refs */
    if (selectedConnection) {
      setForm(sshConnectionToInput(selectedConnection, workspaceId));
    }
  }

  useEffect(() => {
    if (!sessions.length) {
      if (activeSessionId) {
        setActiveSessionId(null);
      }
      return;
    }

    if (!activeSessionId || !sessions.some((session) => session.sessionId === activeSessionId)) {
      setActiveSessionId(sessions[0].sessionId);
    }
  }, [activeSessionId, sessions, setActiveSessionId]);

  const saveMutation = useMutation({
    mutationFn: saveSshConnection,
    onSuccess: (connection) => {
      setSelectedSshConnection(connection.id);
      setDialogOpen(false);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteSshConnection(workspaceId, connectionId),
    onSuccess: () => {
      setSelectedSshConnection(null);
      setDialogOpen(false);
      resetTerminalEvents();
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
      queryClient.invalidateQueries({ queryKey: ["ssh-sessions", workspaceId] });
    },
  });

  const connectMutation = useMutation({
    mutationFn: (connectionId: string) =>
      connectSshSession({ workspaceId, connectionId, cols: 120, rows: 32 }),
    onSuccess: (session) => {
      hydratedSessionIdsRef.current.add(session.sessionId);
      startTerminalSession(session.sessionId, [
        {
          sessionId: session.sessionId,
          kind: "output",
          data: `${t("ssh.session.connected", {
            cols: session.cols,
            host: session.host,
            rows: session.rows,
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

  const cancelReconnectMutation = useMutation({
    mutationFn: (sessionId: string) =>
      cancelSshReconnect({ workspaceId, sessionId }),
    onSuccess: (session) => {
      queryClient.setQueryData<SshSessionSummary[]>(
        ["ssh-sessions", workspaceId],
        (current = []) =>
          current.map((item) => (item.sessionId === session.sessionId ? session : item)),
      );
    },
  });

  const exportMutation = useMutation({
    mutationFn: (sessionId: string) => exportSshLog({ workspaceId, sessionId }),
    onSuccess: (log) => setExportedLog(log.content),
  });

  function updateForm(patch: Partial<SshConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function newConnection() {
    connectMutation.reset();
    deleteMutation.reset();
    saveMutation.reset();
    setSelectedSshConnection(null);
    setForm(defaultSshConnectionInput(workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

  function openConnectionSettings(connection?: SshConnection | null) {
    connectMutation.reset();
    deleteMutation.reset();
    saveMutation.reset();
    const target = connection ?? selectedConnection;
    if (target) {
      setSelectedSshConnection(target.id);
      setForm(sshConnectionToInput(target, workspaceId));
    } else {
      setForm(defaultSshConnectionInput(workspaceId));
    }
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

  function editConnection(connection: SshConnection) {
    connectMutation.reset();
    deleteMutation.reset();
    saveMutation.reset();
    setSelectedSshConnection(connection.id);
    setForm(sshConnectionToInput(connection, workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

  // Keep a stable callback identity for the pushed sidebar so re-renders do not
  // continually replace the shell sidebar node (which would loop via setState
  // in the parent).
  const editConnectionRef = useRef(editConnection);
  useEffect(() => {
    editConnectionRef.current = editConnection;
  });
  const handleEditConnection = useCallback((connection: SshConnection) => {
    editConnectionRef.current(connection);
  }, []);
  const newConnectionRef = useRef(newConnection);
  useEffect(() => {
    newConnectionRef.current = newConnection;
  });
  const handleNewConnection = useCallback(() => {
    newConnectionRef.current();
  }, []);
  const openTerminalTab = useCallback(() => setActiveTab("ssh-main"), [setActiveTab]);

  const shellSidebar = useMemo(
    () => (
      <SshConnectionTree
        active
        collapsed={false}
        onEditConnection={handleEditConnection}
        onNewConnection={handleNewConnection}
        onOpenTerminal={openTerminalTab}
        workspaceId={workspaceId}
      />
    ),
    [handleEditConnection, handleNewConnection, openTerminalTab, workspaceId],
  );

  useEffect(() => {
    if (!onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [onShellSidebarChange, shellSidebar]);

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate({
      ...form,
      workspaceId,
      credentialRef: form.credentialRef?.trim() || null,
      keyPath: form.keyPath?.trim() || null,
    });
  }

  function connectSelectedConnection() {
    connectMutation.reset();
    if (!selectedConnectionId || !selectedConnection) {
      newConnection();
      return;
    }

    const host = selectedConnection.host;
    const port = selectedConnection.port ?? 22;

    // Check if we already trust this host.
    getSshHostFingerprint({ host, port })
      .then((info) => {
        if (info) {
          // Already trusted — connect directly.
          connectMutation.mutate(selectedConnectionId);
        } else {
          // First trust — show confirmation dialog.
          setTrustDialogState({
            open: true,
            connectionId: selectedConnectionId,
            host,
            port,
            fingerprint: null,
            mismatchError: null,
          });
        }
      })
      .catch(() => {
        // If fingerprint check fails, proceed with connection anyway
        // (the backend TOFU will handle it).
        connectMutation.mutate(selectedConnectionId);
      });
  }

  function confirmTrustAndConnect() {
    if (!trustDialogState.connectionId) return;
    connectMutation.reset();
    connectMutation.mutate(trustDialogState.connectionId);
  }

  function retryConnection(connectionId: string) {
    connectMutation.reset();
    connectMutation.mutate(connectionId);
  }

  function requestCloseSession(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    const needsConfirmation =
      session && !["disconnected", "failed"].includes(session.status);
    if (needsConfirmation) {
      setCloseConfirmSessionId(sessionId);
      return;
    }
    closeMutation.mutate(sessionId);
  }

  const closeConfirmSession = closeConfirmSessionId
    ? sessions.find((item) => item.sessionId === closeConfirmSessionId)
    : null;

  function copySessionLog(sessionId: string) {
    const content = terminalEvents
      .filter((event) => event.sessionId === sessionId)
      .map(
        (event) =>
          `[${event.createdAt}] ${event.kind} ${redactTerminalLog(event.data).trim()}`,
      )
      .join("\n");
    if (content) {
      void navigator.clipboard?.writeText(content);
    }
  }

  // Detect host-key mismatch errors from connect failures.
  useEffect(() => {
    const error = connectMutation.error;
    if (!error) return;
    const message = error instanceof Error ? error.message : String(error);
    if (
      message.includes("host key verification failed") ||
      message.includes("fingerprint does not match")
    ) {
      const conn = selectedConnection;
      // eslint-disable-next-line react-hooks/set-state-in-effect -- surfacing mutation error as trust dialog is an external-system sync
      setTrustDialogState({
        open: true,
        connectionId: null,
        host: conn?.host ?? "",
        port: conn?.port ?? 22,
        fingerprint: undefined,
        mismatchError: message,
      });
    }
  }, [connectMutation.error, selectedConnection]);

  // Search has no toolbar button anymore; Ctrl/Cmd+F opens the in-terminal search.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && (event.key === "f" || event.key === "F")) {
        event.preventDefault();
        setSearchOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setSearchOpen]);

  const blockingError = connectionsQuery.error ?? sessionsQuery.error;
  const actionError =
    connectMutation.error ??
    closeMutation.error ??
    cancelReconnectMutation.error ??
    exportMutation.error;

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      {connectionsQuery.isLoading || sessionsQuery.isLoading ? (
        <LoadingState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.state.loadingWorkspace")}
        </LoadingState>
      ) : (
        <TerminalWorkspace
          activeSession={activeSession}
          activeSessionId={activeSessionId}
          actionError={actionError}
          canSplit={sessions.filter((session) => session.status === "connected").length > 1}
          error={blockingError}
          events={terminalEvents}
          emptyMessage={
            connections.length
              ? t("ssh.empty.selectConnection")
              : t("ssh.empty.noConnections")
          }
          onCancelReconnect={(sessionId) => cancelReconnectMutation.mutate(sessionId)}
          onClear={(sessionId) => clearTerminalSessionEvents(sessionId)}
          onCloseSession={requestCloseSession}
          onCopyLog={copySessionLog}
          onExportLog={(sessionId) => exportMutation.mutate(sessionId)}
          onNewConnection={newConnection}
          onNewSession={connectSelectedConnection}
          onOpenPreferences={openConnectionSettings}
          onRetry={retryConnection}
          onSelectSession={setActiveSessionId}
          onSplit={split.setMode}
          selectedConnection={selectedConnection}
          sessions={sessionTabs}
          splitMode={split.mode}
        />
      )}
      <SshConnectionDialog
        canDelete={Boolean(form.id)}
        error={saveMutation.error ?? deleteMutation.error}
        form={form}
        onDelete={() => form.id && deleteMutation.mutate(form.id)}
        onOpenChange={setDialogOpen}
        onSubmit={submitConnection}
        onUpdate={updateForm}
        open={dialogOpen}
        pending={saveMutation.isPending || deleteMutation.isPending}
        workspaceId={workspaceId}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.actions.closeSession")}
        description={
          closeConfirmSession
            ? t("ssh.confirmClose", {
                label: `${closeConfirmSession.username}@${closeConfirmSession.host}`,
              })
            : ""
        }
        onConfirm={() => {
          if (closeConfirmSessionId) {
            closeMutation.mutate(closeConfirmSessionId);
          }
          setCloseConfirmSessionId(null);
        }}
        onOpenChange={(open) => !open && setCloseConfirmSessionId(null)}
        open={closeConfirmSessionId !== null}
        pending={closeMutation.isPending}
        title={t("ssh.session.closeTitle")}
      />
      <HostKeyTrustDialog
        existingFingerprint={trustDialogState.fingerprint}
        host={trustDialogState.host}
        mismatchError={trustDialogState.mismatchError}
        onConfirm={confirmTrustAndConnect}
        onOpenChange={(open) =>
          setTrustDialogState((prev) => ({ ...prev, open }))
        }
        open={trustDialogState.open}
        pending={connectMutation.isPending}
        port={trustDialogState.port}
      />
    </div>
  );
}
