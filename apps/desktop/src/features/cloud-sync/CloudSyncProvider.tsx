import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { hasActiveEntitlement, CLOUD_SYNC_ENTITLEMENT } from "../account/accountEntitlement";
import { useAccount } from "../account/useAccount";
import {
  disableCloudSync,
  enableCloudSync,
  getCloudSyncStatus,
  getGlobalSyncEnabled,
  getLocalWorkspaces,
  replaceDeadLetterWithRemote,
  retryDeadLetterCurrentLocal,
  setGlobalSyncEnabled,
  syncErrorCode,
  syncNow,
} from "./syncApi";
import type { CloudSyncStatus, SyncWorkspaceTarget } from "./syncTypes";
import { CloudSyncContext } from "./useCloudSync";

export function CloudSyncProvider({ children }: { children: ReactNode }) {
  const account = useAccount();
  const hasCloudSyncCapability = account.state.kind === "signedIn"
    && hasActiveEntitlement(account.state.profile.entitlements, CLOUD_SYNC_ENTITLEMENT);
  const available = hasCloudSyncCapability && account.syncContext.kind === "ready";
  const contextErrorCode = account.syncContext.kind === "error" ? account.syncContext.code : null;
  const [revision, setRevision] = useState(0);
  const [statuses, setStatuses] = useState<Map<string, CloudSyncStatus>>(new Map());
  const [globalEnabled, setGlobalEnabledState] = useState(false);
  const [loading, setLoading] = useState(false);
  const [requestErrorCode, setRequestErrorCode] = useState<string | null>(null);
  const [enableTarget, setEnableTarget] = useState<SyncWorkspaceTarget | null>(null);
  const [detailTarget, setDetailTarget] = useState<SyncWorkspaceTarget | null>(null);
  const [cloudWorkspaceDialogOpen, setCloudWorkspaceDialogOpen] = useState(false);
  const [wasAvailable, setWasAvailable] = useState(available);
  const requestId = useRef(0);
  const refresh = useCallback(() => setRevision((value) => value + 1), []);

  const refreshNow = useCallback(async () => {
    if (!available) {
      requestId.current += 1;
      setStatuses(new Map());
      setGlobalEnabledState(false);
      setRequestErrorCode(null);
      setLoading(false);
      return;
    }
    const currentRequest = ++requestId.current;
    setLoading(true);
    try {
      const [workspaceState, enabled] = await Promise.all([
        getLocalWorkspaces(),
        getGlobalSyncEnabled(),
      ]);
      if (currentRequest !== requestId.current) return;
      const entries = await Promise.all(workspaceState.workspaces.map(async (workspace) => [
        workspace.id,
        await getCloudSyncStatus(workspace.id),
      ] as const));
      if (currentRequest !== requestId.current) return;
      setStatuses(new Map(entries));
      setGlobalEnabledState(enabled);
      setRequestErrorCode(null);
    } catch (error) {
      if (currentRequest === requestId.current) setRequestErrorCode(syncErrorCode(error));
    } finally {
      if (currentRequest === requestId.current) setLoading(false);
    }
  }, [available]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- external status fetch owns its pending state; availability loss also invalidates in-flight results
    void refreshNow();
    return () => { requestId.current += 1; };
  }, [refreshNow, revision]);
  useEffect(() => {
    if (!available) return;
    const timer = window.setInterval(() => void refreshNow(), 15_000);
    return () => window.clearInterval(timer);
  }, [available, refreshNow]);

  // Reset before rendering children so revoked access cannot leave a dialog visible.
  if (wasAvailable !== available) {
    setWasAvailable(available);
    if (!available) {
      setEnableTarget(null);
      setDetailTarget(null);
      setCloudWorkspaceDialogOpen(false);
    }
  }

  const runAndRefresh = useCallback(async (operation: () => Promise<void>) => {
    if (!available) {
      const error = { code: contextErrorCode ?? (hasCloudSyncCapability
        ? "cloud_sync_context_unavailable"
        : "cloud_sync_entitlement_required") };
      setRequestErrorCode(error.code);
      throw error;
    }
    setRequestErrorCode(null);
    try {
      await operation();
      await refreshNow();
    } catch (error) {
      const code = syncErrorCode(error);
      setRequestErrorCode(code);
      throw error;
    }
  }, [available, contextErrorCode, hasCloudSyncCapability, refreshNow]);

  const runRecoveryAndRefresh = useCallback(async (operation: () => Promise<void>) => {
    if (!available) {
      const error = { code: contextErrorCode ?? (hasCloudSyncCapability
        ? "cloud_sync_context_unavailable"
        : "cloud_sync_entitlement_required") };
      setRequestErrorCode(error.code);
      throw error;
    }
    setRequestErrorCode(null);
    try {
      await operation();
    } catch (error) {
      setRequestErrorCode(syncErrorCode(error));
      throw error;
    } finally {
      await refreshNow();
    }
  }, [available, contextErrorCode, hasCloudSyncCapability, refreshNow]);

  const errorCode = contextErrorCode ?? requestErrorCode;
  const visibleStatuses = useMemo(
    () => available ? statuses : new Map<string, CloudSyncStatus>(),
    [available, statuses],
  );

  const value = useMemo(() => ({
    cloudWorkspaceDialogOpen,
    detailTarget,
    enableTarget,
    available,
    hasCloudSyncCapability,
    errorCode,
    globalEnabled: available && globalEnabled,
    loading,
    revision,
    statuses: visibleStatuses,
    closeCloudWorkspaceDialog: () => setCloudWorkspaceDialogOpen(false),
    closeDetailDialog: () => setDetailTarget(null),
    closeEnableDialog: () => setEnableTarget(null),
    enableWorkspace: (workspaceId: string) => runAndRefresh(() => enableCloudSync(workspaceId)),
    openCloudWorkspaceDialog: () => { if (available) setCloudWorkspaceDialogOpen(true); },
    openDetailDialog: (target: SyncWorkspaceTarget) => { if (available) setDetailTarget(target); },
    openEnableDialog: (target: SyncWorkspaceTarget) => { if (available) setEnableTarget(target); },
    pauseWorkspace: (workspaceId: string) => runAndRefresh(() => disableCloudSync(workspaceId)),
    refresh,
    refreshNow,
    retryDeadLetter: (workspaceId: string, operationId: string) => runRecoveryAndRefresh(() => retryDeadLetterCurrentLocal(workspaceId, operationId)),
    retryWorkspace: (workspaceId: string) => runAndRefresh(() => syncNow(workspaceId)),
    setServiceEnabled: (enabled: boolean) => runAndRefresh(() => setGlobalSyncEnabled(enabled)),
    replaceDeadLetterWithRemote: (workspaceId: string, operationId: string) => runRecoveryAndRefresh(() => replaceDeadLetterWithRemote(workspaceId, operationId)),
  }), [available, cloudWorkspaceDialogOpen, detailTarget, enableTarget, errorCode, globalEnabled, hasCloudSyncCapability, loading, refresh, refreshNow, revision, runAndRefresh, runRecoveryAndRefresh, visibleStatuses]);
  return <CloudSyncContext.Provider value={value}>{children}</CloudSyncContext.Provider>;
}
