import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react";
import type { SshConnection } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { ConfirmDialog, useI18n } from "@unfour/ui";
import { TerminalWorkspace } from "./components/TerminalWorkspace";
import { SshConnectionTree } from "./components/SshConnectionTree";
import { SshConnectionDialog } from "./components/SshConnectionDialog";
import { HostKeyTrustDialog } from "./components/HostKeyTrustDialog";
import { useSshConnections } from "./hooks/useSshConnections";
import { useSshConnectionManagement } from "./hooks/useSshConnectionManagement";
import { useTerminalSessionOrchestration } from "./hooks/useTerminalSessionOrchestration";
import { useTerminalSplit } from "./hooks/useTerminalSplit";
import { useTerminalStore } from "./model/terminal-state";

export function SshConnectionsPage({
  active = true,
  sidebarActive = active,
  onShowSaved,
  onOpenTasks,
  onShellSidebarChange,
  workspaceId,
}: {
  active?: boolean;
  sidebarActive?: boolean;
  onShowSaved?: () => void;
  onOpenTasks?: () => void;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const setActiveTab = useWorkspaceStore((state) => state.setActiveTab);
  const activateWorkspace = useTerminalStore((state) => state.activateWorkspace);
  const setSearchOpen = useTerminalStore((state) => state.setSearchOpen);
  const split = useTerminalSplit();
  const workspaceScope = useRef<string | null>(workspaceId);
  const connectionsQuery = useSshConnections(workspaceId, { active });
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const terminal = useTerminalSessionOrchestration({
    active,
    connections,
    workspaceId,
    workspaceScope,
  });

  useEffect(() => {
    activateWorkspace(workspaceId);
  }, [activateWorkspace, workspaceId]);

  const connection = useSshConnectionManagement({
    connections,
    connectMutation: terminal.connectMutation,
    workspaceId,
  });
  const { activeSessionId, setActiveSessionId, visibleSessions } = terminal;

  // Reconcile after workspace activation, preserving the store's workspace slice lifecycle.
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

  useEffect(() => {
    workspaceScope.current = workspaceId;
    return () => { workspaceScope.current = null; };
  }, [workspaceId]);

  // Keep a stable callback identity for the pushed sidebar so re-renders do not
  // continually replace the shell sidebar node (which would loop via setState
  // in the parent).
  const editConnectionRef = useRef(connection.editConnection);
  useEffect(() => {
    editConnectionRef.current = connection.editConnection;
  });
  const handleEditConnection = useCallback((connection: SshConnection) => {
    onShowSaved?.();
    editConnectionRef.current(connection);
  }, [onShowSaved]);
  const newConnectionRef = useRef(connection.newConnection);
  useEffect(() => {
    newConnectionRef.current = connection.newConnection;
  });
  const handleNewConnection = useCallback(() => {
    onShowSaved?.();
    newConnectionRef.current();
  }, [onShowSaved]);
  const openTerminalTab = useCallback(() => {
    onShowSaved?.();
    setActiveTab("ssh-main");
  }, [onShowSaved, setActiveTab]);
  const openTasks = useCallback(() => {
    onShowSaved?.();
    onOpenTasks?.();
  }, [onShowSaved, onOpenTasks]);

  const shellSidebar = useMemo(
    () => (
      <SshConnectionTree
        active
        collapsed={false}
        onEditConnection={handleEditConnection}
        onNewConnection={handleNewConnection}
        onOpenTasks={onOpenTasks ? openTasks : undefined}
        onOpenTerminal={openTerminalTab}
        workspaceId={workspaceId}
      />
    ),
    [handleEditConnection, handleNewConnection, onOpenTasks, openTasks, openTerminalTab, workspaceId],
  );

  useEffect(() => {
    if (!sidebarActive || !onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [sidebarActive, onShellSidebarChange, shellSidebar]);

  // Search has no toolbar button anymore; Ctrl/Cmd+F opens the in-terminal search.
  useEffect(() => {
    if (!active) return;
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && (event.key === "f" || event.key === "F")) {
        event.preventDefault();
        setSearchOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active, setSearchOpen]);

  const blockingError = connectionsQuery.error ?? terminal.sessionsQuery.error;

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-surface)]">
      <TerminalWorkspace
        activeSession={terminal.activeSession}
        activeSessionId={terminal.activeSessionId}
        actionError={terminal.actionError}
        connecting={terminal.connectMutation.isPending}
        error={blockingError}
        events={terminal.terminalEvents}
        emptyMessage={
          connections.length
            ? t("ssh.empty.selectConnection")
            : t("ssh.empty.noConnections")
        }
        loading={connectionsQuery.isLoading}
        onClear={(sessionId) => terminal.clearTerminalSessionEvents(sessionId)}
        onCloseAll={terminal.closeAllSessions}
        onCloseLeft={terminal.closeSessionsToLeft}
        onCloseOthers={terminal.closeOtherSessions}
        onCloseRight={terminal.closeSessionsToRight}
        onCloseSession={terminal.requestCloseSession}
        onDuplicate={terminal.openFromSession}
        onNewConnection={connection.newConnection}
        onNewSession={connection.connectSelectedConnection}
        onOpenPreferences={connection.openConnectionSettings}
        onReconnect={terminal.reconnectSession}
        onRetry={terminal.openFromSession}
        onSelectSession={terminal.setActiveSessionId}
        selectedConnection={connection.selectedConnection}
        sessions={terminal.sessionTabs}
        splitMode={split.mode}
        surfaceActive={active}
      />
      <SshConnectionDialog
        externalChange={connection.externalChange}
        onReload={connection.reloadForm}
        canTest={connection.canTest}
        error={connection.saveMutation.error}
        form={connection.form}
        onOpenChange={connection.changeDialogOpen}
        onSubmit={connection.submitConnection}
        onTest={connection.testConnection}
        onUpdate={connection.updateForm}
        open={connection.dialogOpen}
        pending={connection.saveMutation.isPending}
        testing={connection.testMutation.isPending}
        testResult={connection.testResult}
        onCloseTestResult={connection.closeTestResult}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.actions.closeSession")}
        description={
          terminal.closeConfirmSession
            ? t("ssh.confirmClose", {
                label: `${terminal.closeConfirmSession.username}@${terminal.closeConfirmSession.host}`,
              })
            : ""
        }
        onConfirm={terminal.confirmCloseSession}
        onOpenChange={(open) => !open && terminal.setCloseConfirmSessionId(null)}
        open={terminal.closeConfirmSessionId !== null}
        pending={terminal.closeMutation.isPending}
        title={t("ssh.session.closeTitle")}
      />
      <ConfirmDialog
        confirmLabel={t("ssh.actions.closeSessions")}
        description={
          terminal.batchCloseRequest
            ? t("ssh.confirmCloseMany", {
                count: terminal.batchCloseRequest.sessionIds.length,
                labels: terminal.batchCloseRequest.labels.join(", "),
              })
            : ""
        }
        onConfirm={terminal.confirmBatchClose}
        onOpenChange={(open) => !open && terminal.setBatchCloseRequest(null)}
        open={terminal.batchCloseRequest !== null}
        pending={terminal.closeMutation.isPending}
        title={t("ssh.session.closeManyTitle")}
      />
      <HostKeyTrustDialog
        existingFingerprint={connection.trustDialogState.fingerprint}
        host={connection.trustDialogState.host}
        mismatchError={connection.trustDialogState.mismatchError}
        onConfirm={connection.confirmTrustAndConnect}
        onOpenChange={(open) =>
          connection.setTrustDialogState((prev) => ({ ...prev, open }))
        }
        onResetAndReconnect={connection.resetHostAndReconnect}
        open={connection.trustDialogState.open}
        pending={terminal.connectMutation.isPending}
        port={connection.trustDialogState.port}
        resetPending={connection.resetHostKeyMutation.isPending}
      />
    </div>
  );
}
