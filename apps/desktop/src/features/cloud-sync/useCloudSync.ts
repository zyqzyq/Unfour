import { createContext, useContext } from "react";
import type { CloudSyncStatus, SyncWorkspaceTarget } from "./syncTypes";

export interface CloudSyncContextValue {
  cloudWorkspaceDialogOpen: boolean;
  detailTarget: SyncWorkspaceTarget | null;
  enableTarget: SyncWorkspaceTarget | null;
  available: boolean;
  hasCloudSyncCapability: boolean;
  errorCode: string | null;
  globalEnabled: boolean;
  loading: boolean;
  revision: number;
  statuses: ReadonlyMap<string, CloudSyncStatus>;
  workspaceErrors: ReadonlyMap<string, string>;
  closeCloudWorkspaceDialog(): void;
  closeDetailDialog(): void;
  closeEnableDialog(): void;
  enableWorkspace(workspaceId: string): Promise<void>;
  openCloudWorkspaceDialog(): void;
  openDetailDialog(workspace: SyncWorkspaceTarget): void;
  openEnableDialog(workspace: SyncWorkspaceTarget): void;
  pauseWorkspace(workspaceId: string): Promise<void>;
  refresh(): void;
  refreshNow(): Promise<void>;
  retryDeadLetter(workspaceId: string, operationId: string): Promise<void>;
  retryWorkspace(workspaceId: string): Promise<void>;
  setServiceEnabled(enabled: boolean): Promise<void>;
  replaceDeadLetterWithRemote(workspaceId: string, operationId: string): Promise<void>;
}

export const CloudSyncContext = createContext<CloudSyncContextValue | null>(null);

export function useCloudSync(): CloudSyncContextValue {
  const value = useContext(CloudSyncContext);
  if (!value) throw new Error("useCloudSync must be used inside CloudSyncProvider");
  return value;
}
