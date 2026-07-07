import { FormEvent, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  closeSshSession,
  connectSshSession,
  exportSshLog,
  getSshHostFingerprint,
  getSshSessionHistory,
  registerSshTerminalChannel,
  saveSshConnection,
  testSshConnection,
  type SshConnection,
  type SshConnectionInput,
  type SshHostFingerprintInfo,
  type SshSessionEvent,
  type SshSessionSummary,
  type SshTerminalDataPayload,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { ConfirmDialog, LoadingState, useI18n } from "@unfour/ui";
import { TerminalWorkspace } from "./components/TerminalWorkspace";
import { SshConnectionTree } from "./components/SshConnectionTree";
import { SshConnectionDialog } from "./components/SshConnectionDialog";
import { SshTestResultDialog } from "./components/SshTestResultDialog";
import { HostKeyTrustDialog } from "./components/HostKeyTrustDialog";
import { useSshConnections } from "./hooks/useSshConnections";
import { useTerminalSessions } from "./hooks/useTerminalSessions";
import { useTerminalSplit } from "./hooks/useTerminalSplit";
import { useTerminalStore } from "./model/terminal-state";
import {
  defaultSshConnectionInput,
  sshConnectionToInput,
} from "./model/ssh-connection-state";
import { buildTerminalSessionTabs, shouldShowTerminalSessionTab } from "./model/terminal-tabs";
import { formatTerminalError } from "./model/errors";

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
  const addFrontendFailedSession = useTerminalStore(
    (state) => state.addFrontendFailedSession,
  );
  const appendTerminalEvents = useTerminalStore((state) => state.appendTerminalEvents);
  const clearTerminalSessionEvents = useTerminalStore(
    (state) => state.clearTerminalSessionEvents,
  );
  const dismissSession = useTerminalStore((state) => state.dismissSession);
  const dismissedSessionIds = useTerminalStore((state) => state.dismissedSessionIds);
  const frontendFailedSessions = useTerminalStore(
    (state) => state.frontendFailedSessions,
  );
  const hydrateTerminalSession = useTerminalStore(
    (state) => state.hydrateTerminalSession,
  );
  const setActiveSessionId = useTerminalStore((state) => state.setActiveSessionId);
  const setExportedLog = useTerminalStore((state) => state.setExportedLog);
  const setSearchOpen = useTerminalStore((state) => state.setSearchOpen);
  const startTerminalSession = useTerminalStore((state) => state.startTerminalSession);
  const terminalEvents = useTerminalStore((state) => state.terminalEvents);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"new" | "edit" | null>(null);
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
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(
    null,
  );

  const connectionsQuery = useSshConnections(workspaceId);
  const sessionsQuery = useTerminalSessions(workspaceId);
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
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
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  const prevSelectedConnectionIdRef = useRef(selectedConnectionId);
  const activeSession = useMemo(
    () => visibleSessions.find((item) => item.sessionId === activeSessionId) ?? null,
    [activeSessionId, visibleSessions],
  );
  const sessionTabs = useMemo(
    () => buildTerminalSessionTabs({ connections, sessions: visibleSessions }),
    [connections, visibleSessions],
  );

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    // Live output is coalesced into the store roughly once per frame. It arrives
    // over a Tauri IPC channel (not the event system, which stalls under a
    // full-screen-redraw emit burst on WebView2/Windows) and is batched here so
    // a keystroke echo does not force a full re-render per chunk.
    let pending: SshSessionEvent[] = [];
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flushPending = () => {
      flushTimer = null;
      if (!pending.length) {
        return;
      }
      const batch = pending;
      pending = [];
      appendTerminalEvents(batch);
    };
    const handlePayload = (payload: SshTerminalDataPayload) => {
      if (!payload?.sessionId) {
        return;
      }
      if (payload.data) {
        pending.push({
          sessionId: payload.sessionId,
          kind:
            payload.status === "disconnected" || payload.status === "failed"
              ? "close"
              : "output",
          data: payload.data,
          createdAt: new Date().toISOString(),
        });
        if (flushTimer === null) {
          flushTimer = setTimeout(flushPending, 16);
        }
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
    };
    registerSshTerminalChannel(handlePayload)
      .then((d) => {
        if (disposed) {
          d();
        } else {
          dispose = d;
        }
      })
      .catch(() => {
        // Browser mock mode has no Tauri IPC; query polling remains active.
      });
    return () => {
      disposed = true;
      if (flushTimer !== null) {
        clearTimeout(flushTimer);
      }
      flushPending();
      dispose?.();
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

    if (dialogMode === "new") {
      return;
    }

    if (!selectedConnectionId || !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedSshConnection(connections[0].id);
    }
  }, [connections, selectedConnectionId, setSelectedSshConnection, dialogMode]);

  // Sync form state when the selected connection changes (render-time adjustment pattern).
  /* eslint-disable react-hooks/refs -- render-time ref read/write is React's recommended pattern for adjusting state when a derived value changes */
  if (selectedConnectionId !== prevSelectedConnectionIdRef.current) {
    prevSelectedConnectionIdRef.current = selectedConnectionId;
    /* eslint-enable react-hooks/refs */
    if (selectedConnection && dialogMode !== "new") {
      setForm(sshConnectionToInput(selectedConnection, workspaceId));
    }
  }

  useEffect(() => {
    if (!visibleSessions.length) {
      if (activeSessionId) {
        setActiveSessionId(null);
      }
      return;
    }

    if (
      !activeSessionId ||
      !visibleSessions.some((session) => session.sessionId === activeSessionId)
    ) {
      setActiveSessionId(visibleSessions[0].sessionId);
    }
  }, [activeSessionId, visibleSessions, setActiveSessionId]);

  const saveMutation = useMutation({
    mutationFn: saveSshConnection,
    onSuccess: (connection) => {
      setSelectedSshConnection(connection.id);
      setDialogOpen(false);
      setDialogMode(null);
      queryClient.invalidateQueries({ queryKey: ["ssh-connections", workspaceId] });
    },
  });

  // Test a connection using the dedicated test endpoint, which accepts the full
  // form payload. The backend resolves stored credentials via `credential_ref`
  // (carried on the form for saved connections) and applies `secret` as an
  // override, so a newly typed password/passphrase is honored as well. The
  // endpoint spins up and tears down its own throwaway session.
  const testMutation = useMutation({
    mutationFn: async ({
      form,
    }: {
      form: SshConnectionInput;
    }) => {
      return testSshConnection({ ...form, workspaceId });
    },
    onSuccess: (result) =>
      setTestResult({ ok: result.ok, message: result.message }),
    onError: (error) =>
      setTestResult({ ok: false, message: formatTerminalError(error) }),
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
    onError: (error, connectionId) => {
      const connection = connections.find((c) => c.id === connectionId);
      if (!connection) return;
      const syntheticId = `__frontend_failed_${connectionId}_${Date.now()}`;
      const now = new Date().toISOString();
      const failedSession: SshSessionSummary = {
        sessionId: syntheticId,
        workspaceId,
        connectionId,
        status: "disconnected",
        reconnectAttempt: 0,
        authKind: connection.authKind,
        host: connection.host,
        username: connection.username,
        cols: 120,
        rows: 32,
        createdAt: now,
        updatedAt: now,
      };
      const errorMessage = formatTerminalError(error);
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

  const exportMutation = useMutation({
    mutationFn: (sessionId: string) => exportSshLog({ workspaceId, sessionId }),
    onSuccess: (log) => setExportedLog(log.content),
  });

  function updateForm(patch: Partial<SshConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function newConnection() {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    setSelectedSshConnection(null);
    setForm(defaultSshConnectionInput(workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogMode("new");
    setDialogOpen(true);
  }

  function openConnectionSettings(connection?: SshConnection | null) {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    const target = connection ?? selectedConnection;
    if (target) {
      setSelectedSshConnection(target.id);
      setForm(sshConnectionToInput(target, workspaceId));
      setDialogMode("edit");
    } else {
      setForm(defaultSshConnectionInput(workspaceId));
      setDialogMode("new");
    }
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogOpen(true);
  }

  function editConnection(connection: SshConnection) {
    connectMutation.reset();
    testMutation.reset();
    setTestResult(null);
    saveMutation.reset();
    setSelectedSshConnection(connection.id);
    setForm(sshConnectionToInput(connection, workspaceId));
    setTrustDialogState((prev) => ({ ...prev, open: false }));
    setDialogMode("edit");
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
      // Preserve the secret verbatim (passwords may contain spaces); an empty
      // field means "keep the saved password".
      secret: form.secret ? form.secret : null,
    });
  }

  function testConnection() {
    testMutation.reset();
    setTestResult(null);
    testMutation.mutate({ form });
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
    getSshHostFingerprint({ workspaceId, host, port })
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
    // Already disconnected/failed: just drop the tab from view. The backend
    // retains it as history, so re-closing would be a no-op.
    if (session) {
      closeMutation.mutate(sessionId);
    }
    dismissSession(sessionId);
  }

  const closeConfirmSession = closeConfirmSessionId
    ? sessions.find((item) => item.sessionId === closeConfirmSessionId)
    : null;

  // Close a session without the confirmation prompt — used by the batch tab
  // actions (close others/all/left/right) where a dialog per tab would be noise.
  function closeSessionNow(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    if (session) {
      closeMutation.mutate(sessionId);
    }
    dismissSession(sessionId);
  }

  function reconnectSession(sessionId: string) {
    const session = sessions.find((item) => item.sessionId === sessionId);
    if (!session) {
      return;
    }
    closeSessionNow(sessionId);
    retryConnection(session.connectionId);
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
    connectMutation.error ?? closeMutation.error ?? exportMutation.error;

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
          connecting={connectMutation.isPending}
          error={blockingError}
          events={terminalEvents}
          emptyMessage={
            connections.length
              ? t("ssh.empty.selectConnection")
              : t("ssh.empty.noConnections")
          }
          onClear={(sessionId) => clearTerminalSessionEvents(sessionId)}
          onCloseAll={closeAllSessions}
          onCloseLeft={closeSessionsToLeft}
          onCloseOthers={closeOtherSessions}
          onCloseRight={closeSessionsToRight}
          onCloseSession={requestCloseSession}
          onDuplicate={retryConnection}
          onNewConnection={newConnection}
          onNewSession={connectSelectedConnection}
          onOpenPreferences={openConnectionSettings}
          onReconnect={reconnectSession}
          onRetry={retryConnection}
          onSelectSession={setActiveSessionId}
          selectedConnection={selectedConnection}
          sessions={sessionTabs}
          splitMode={split.mode}
        />
      )}
      <SshConnectionDialog
        canTest={
          Boolean(form.host?.trim()) &&
          Boolean(form.username?.trim()) &&
          (form.authKind !== "private-key" || Boolean(form.keyPath?.trim())) &&
          (form.authKind !== "password" ||
            Boolean(form.id) ||
            Boolean(form.secret?.trim()))
        }
        error={saveMutation.error}
        form={form}
        onOpenChange={(open) => {
          setDialogOpen(open);
          if (!open) {
            setDialogMode(null);
            setTestResult(null);
          }
        }}
        onSubmit={submitConnection}
        onTest={testConnection}
        onUpdate={updateForm}
        open={dialogOpen}
        pending={saveMutation.isPending}
        testing={testMutation.isPending}
      />
      <SshTestResultDialog
        onOpenChange={(open) => !open && setTestResult(null)}
        result={testResult}
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
            dismissSession(closeConfirmSessionId);
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
