import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../account/accountApi";
import type { CloudSyncStatus, CloudWorkspace, DownloadDecision, LocalWorkspaceState, SyncConflict, SyncDiagnostics, SyncEntityType } from "./syncTypes";

const mockEnabled = new Set<string>();
let mockGlobalEnabled = false;

export function getCloudSyncStatus(workspaceId: string): Promise<CloudSyncStatus> {
  if (!isTauriRuntime()) {
    const enabled = mockEnabled.has(workspaceId);
    return Promise.resolve({
      binding: enabled ? {
        accountId: "preview-account",
        localWorkspaceId: workspaceId,
        cloudWorkspaceId: `cloud-${workspaceId}`,
        lastPulledCursor: 1,
        syncEnabled: true,
        state: "active",
        initialCursor: 0,
        initialTotal: 1,
        initialConfirmed: 1,
        initializationCheckpoint: "preview-operation",
        sshTaskV3BootstrapState: "completed",
        connectionV4BootstrapState: "completed",
        generation: 0,
        lastSuccessAt: new Date().toISOString(),
        lastError: null,
        consecutiveFailureCount: 0,
      } : null,
      pendingCount: 0,
      uncertainCount: 0,
      inFlightCount: 0,
      deadCount: 0,
      deadLetters: [],
      conflictCount: 0,
      running: false,
    });
  }
  return invoke("cloud_sync_status", { workspaceId });
}

export function getLocalWorkspaces(): Promise<LocalWorkspaceState> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      activeWorkspaceId: "preview-workspace",
      workspaces: [
        { id: "preview-workspace", name: "Preview workspace" },
        { id: "local-workspace", name: "Local workspace" },
      ],
    });
  }
  return invoke("workspace_list");
}

export function getGlobalSyncEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) return Promise.resolve(mockGlobalEnabled);
  return invoke("cloud_sync_global_status");
}

export async function setGlobalSyncEnabled(enabled: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    mockGlobalEnabled = enabled;
    return;
  }
  await invoke("cloud_sync_set_global_enabled", { enabled });
}

export function getSyncDiagnostics(workspaceId: string): Promise<SyncDiagnostics | null> {
  if (!isTauriRuntime()) {
    return getCloudSyncStatus(workspaceId).then((status) => status.binding ? ({
      localWorkspaceId: workspaceId,
      remoteWorkspaceId: status.binding.cloudWorkspaceId,
      lastPushAt: status.binding.lastSuccessAt,
      lastPullAt: status.binding.lastSuccessAt,
      pendingOutboxCount: status.pendingCount + status.uncertainCount + status.inFlightCount + status.deadCount,
      deadOutboxCount: status.deadCount,
      deadLetters: status.deadLetters,
      pullCursor: status.binding.lastPulledCursor,
      lastErrorCode: status.binding.lastError,
      consecutiveFailureCount: status.binding.consecutiveFailureCount,
      nextRetryAt: null,
      lastServerErrorCode: null,
      lastServerRequestId: null,
      lastHttpStatus: null,
      lastSyncPhase: null,
      recentEvents: [],
    }) : null);
  }
  return invoke("cloud_sync_diagnostics", { workspaceId });
}

export async function enableCloudSync(workspaceId: string): Promise<void> {
  if (!isTauriRuntime()) {
    mockEnabled.add(workspaceId);
    return;
  }
  await invoke("cloud_sync_enable", { workspaceId });
}

export async function disableCloudSync(workspaceId: string): Promise<void> {
  if (!isTauriRuntime()) {
    mockEnabled.delete(workspaceId);
    return;
  }
  await invoke("cloud_sync_disable", { workspaceId });
}

export async function syncNow(workspaceId: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_now", { workspaceId });
}

export async function retryDeadLetterCurrentLocal(
  workspaceId: string,
  operationId: string,
): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_retry_dead_letter_current_local", { workspaceId, operationId });
}

export async function replaceDeadLetterWithRemote(
  workspaceId: string,
  operationId: string,
): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_use_remote_dead_letter", { workspaceId, operationId });
}

export async function syncAll(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_all");
}

export function listCloudWorkspaces(): Promise<CloudWorkspace[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([{
      cloudWorkspaceId: "cloud-preview-workspace",
      rootEntityId: "remote-preview-workspace",
      name: "Remote preview workspace",
      currentCursor: 4,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    }]);
  }
  return invoke("cloud_sync_list_workspaces");
}

export async function downloadCloudWorkspace(cloudWorkspaceId: string, decision: DownloadDecision): Promise<string> {
  if (!isTauriRuntime()) return `downloaded-${cloudWorkspaceId}`;
  return invoke("cloud_sync_download", { cloudWorkspaceId, decision });
}

export function listSyncConflicts(workspaceId: string): Promise<SyncConflict[]> {
  if (!isTauriRuntime()) return Promise.resolve([]);
  return invoke("cloud_sync_conflicts", { workspaceId });
}

export async function keepLocalConflict(
  workspaceId: string,
  entityType: SyncEntityType,
  entityId: string,
): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_keep_local", { workspaceId, entityType, entityId });
}

export async function useRemoteConflict(
  workspaceId: string,
  entityType: SyncEntityType,
  entityId: string,
): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("cloud_sync_use_remote", { workspaceId, entityType, entityId });
}

export function syncErrorCode(error: unknown): string {
  if (typeof error === "string" && /^cloud_sync_[a-z_]+$/.test(error)) return error;
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as { code?: unknown }).code ?? "cloud_sync_failed");
  }
  return "cloud_sync_failed";
}
