import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
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
  type SshConnectionInput,
  type SshHostFingerprintInfo,
  type SshSessionSummary,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { LoadingState, useI18n } from "@unfour/ui";
import { TerminalModuleToolbar } from "./components/TerminalModuleToolbar";
import { TerminalWorkspace } from "./components/TerminalWorkspace";
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

export function TerminalPage({ workspaceId }: { workspaceId: string }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    selectedSshConnectionId: selectedConnectionId,
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
  const selectedConnectionSession = useMemo(() => {
    if (!selectedConnectionId) {
      return null;
    }
    return (
      sessions.find(
        (item) =>
          item.connectionId === selectedConnectionId &&
          ["connected", "degraded", "reconnecting"].includes(item.status),
      ) ??
      sessions.find((item) => item.connectionId === selectedConnectionId) ??
      null
    );
  }, [selectedConnectionId, sessions]);
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
  const selectedConnectionStatus = useMemo(() => {
    if (connectMutation.isPending) {
      return "connecting" as const;
    }
    if (connectMutation.error && connectMutation.variables === selectedConnectionId) {
      return "failed" as const;
    }
    return selectedConnectionSession?.status ?? "disconnected";
  }, [
    connectMutation.error,
    connectMutation.isPending,
    connectMutation.variables,
    selectedConnectionId,
    selectedConnectionSession?.status,
  ]);

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
    mutationFn: () => exportSshLog({ workspaceId, sessionId: activeSessionId ?? "" }),
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

  function openConnectionSettings() {
    connectMutation.reset();
    deleteMutation.reset();
    saveMutation.reset();
    if (selectedConnection) {
      setForm(sshConnectionToInput(selectedConnection, workspaceId));
    } else {
      setForm(defaultSshConnectionInput(workspaceId));
    }
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

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

  function requestCloseSession(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    const needsConfirmation =
      session && !["disconnected", "failed"].includes(session.status);
    if (needsConfirmation) {
      const label = `${session.username}@${session.host}`;
      const confirmed = window.confirm(t("ssh.confirmClose", { label }));
      if (!confirmed) {
        return;
      }
    }
    closeMutation.mutate(sessionId);
  }

  function copyActiveSessionLog() {
    if (!activeSessionId) {
      return;
    }
    const content = terminalEvents
      .filter((event) => event.sessionId === activeSessionId)
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

  const blockingError = connectionsQuery.error ?? sessionsQuery.error;
  const actionError =
    connectMutation.error ??
    closeMutation.error ??
    cancelReconnectMutation.error ??
    exportMutation.error;

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <TerminalModuleToolbar
        activeSessionCount={sessions.length}
        canConnect={Boolean(selectedConnectionId)}
        canSplit={sessions.filter((session) => session.status === "connected").length > 1}
        canUseSessionActions={Boolean(activeSessionId)}
        connecting={connectMutation.isPending}
        reconnecting={
          activeSession?.status === "degraded" || activeSession?.status === "reconnecting"
        }
        onCancelReconnect={() =>
          activeSessionId && cancelReconnectMutation.mutate(activeSessionId)
        }
        onClear={() => clearTerminalSessionEvents(activeSessionId)}
        onCloseSession={() => activeSessionId && requestCloseSession(activeSessionId)}
        onCopyLog={copyActiveSessionLog}
        onExportLog={() => exportMutation.mutate()}
        onNewConnection={newConnection}
        onNewSession={connectSelectedConnection}
        onOpenPreferences={openConnectionSettings}
        onSearch={() => setSearchOpen(true)}
        onSplit={split.setMode}
        selectedConnectionName={selectedConnection?.name}
        splitMode={split.mode}
      />
      {connectionsQuery.isLoading || sessionsQuery.isLoading ? (
        <LoadingState className="min-h-0 flex-1 rounded-none border-0">
          {t("ssh.state.loadingWorkspace")}
        </LoadingState>
      ) : (
        <TerminalWorkspace
          activeSession={activeSession}
          activeSessionId={activeSessionId}
          actionError={actionError}
          canStartSession={Boolean(selectedConnectionId)}
          error={blockingError}
          events={terminalEvents}
          emptyMessage={
            connections.length
              ? t("ssh.empty.selectConnection")
              : t("ssh.empty.noConnections")
          }
          onCloseSession={requestCloseSession}
          onNewConnection={newConnection}
          onNewSession={connectSelectedConnection}
          onSelectSession={setActiveSessionId}
          selectedConnection={selectedConnection}
          selectedConnectionStatus={selectedConnectionStatus}
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
