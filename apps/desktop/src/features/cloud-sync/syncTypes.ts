export type SyncEntityType =
  | "workspace"
  | "connection"
  | "workspaceVariable"
  | "workspaceEnvironment"
  | "workspaceEnvironmentVariable"
  | "apiCollection"
  | "apiFolder"
  | "apiRequest"
  | "sshTask"
  | "sshTaskStep";

export interface SyncBinding {
  accountId: string;
  localWorkspaceId: string;
  cloudWorkspaceId: string;
  lastPulledCursor: number;
  syncEnabled: boolean;
  state: "preparing" | "uploading" | "downloading" | "reconciling" | "active" | "paused" | "conflict" | "error";
  initialCursor: number | null;
  initialTotal: number;
  initialConfirmed: number;
  initializationCheckpoint: string | null;
  sshTaskV3BootstrapState: "pending" | "completed";
  connectionV4BootstrapState: "pending" | "completed";
  generation: number;
  lastSuccessAt: string | null;
  lastError: string | null;
  consecutiveFailureCount: number;
}

export type CloudSyncViewState =
  | "local_only"
  | "syncing"
  | "synced"
  | "paused"
  | "offline"
  | "auth_required"
  | "capability_required"
  | "attention";

export interface SyncWorkspaceTarget {
  id: string;
  name: string;
}

export interface LocalWorkspace {
  id: string;
  name: string;
}

export interface LocalWorkspaceState {
  activeWorkspaceId: string;
  workspaces: LocalWorkspace[];
}

export interface SyncDiagnostics {
  localWorkspaceId: string;
  remoteWorkspaceId: string;
  lastPushAt: string | null;
  lastPullAt: string | null;
  pendingOutboxCount: number;
  deadOutboxCount: number;
  deadLetters: DeadLetter[];
  pullCursor: number;
  lastErrorCode: string | null;
  consecutiveFailureCount: number;
  nextRetryAt: string | null;
  lastServerErrorCode: string | null;
  lastServerRequestId: string | null;
  lastHttpStatus: number | null;
  lastSyncPhase: string | null;
  recentEvents: SyncDiagnosticEvent[];
}

export interface SyncDiagnosticEvent {
  source: "domain" | "local" | "remote" | null;
  category: string;
  errorCode: string;
  requestId: string | null;
  httpStatus: number | null;
  phase: string | null;
  operationId: string | null;
  operationIndex: number | null;
  entityType: string | null;
  entityId: string | null;
  occurredAt: string;
}

export interface DeadLetter {
  operationId: string;
  entityType: SyncEntityType;
  entityId: string;
  entityName: string | null;
  errorCode: string;
}

export interface CloudSyncStatus {
  binding: SyncBinding | null;
  pendingCount: number;
  uncertainCount: number;
  inFlightCount: number;
  deadCount: number;
  deadLetters: DeadLetter[];
  conflictCount: number;
  running: boolean;
}

export interface CloudWorkspace {
  cloudWorkspaceId: string;
  rootEntityId: string;
  name: string | null;
  currentCursor: number;
  createdAt: string;
  updatedAt: string;
}

export type DownloadDecision = "cancel" | "downloadToNewWorkspace";

export interface SyncConflict {
  cloudWorkspaceId: string;
  entityType: SyncEntityType;
  entityId: string;
  serverVersion: number;
  operation: "upsert" | "delete";
  localPayload: Record<string, unknown> | null;
  remotePayload: Record<string, unknown> | null;
  localSecretPresent: boolean | null;
}

export interface SyncCommandError {
  code?: string;
  message?: string;
}
