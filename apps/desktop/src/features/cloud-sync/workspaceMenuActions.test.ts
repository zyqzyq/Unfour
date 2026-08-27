import { describe, expect, it, vi } from "vitest";
import type { Workspace } from "@unfour/command-client";
import type { CloudSyncStatus } from "./syncTypes";
import type { CloudSyncContextValue } from "./useCloudSync";
import { createCloudSyncWorkspaceActions } from "./workspaceMenuActions";

const workspace: Workspace = { id: "workspace", name: "Backend", environmentType: "dev", mcpPolicy: "auto", isDefault: false, lastOpenedAt: null, createdAt: "", updatedAt: "", deletedAt: null, revision: 1 };
const emptyStatus: CloudSyncStatus = { binding: null, pendingCount: 0, uncertainCount: 0, inFlightCount: 0, deadCount: 0, deadLetters: [], conflictCount: 0, running: false };

function sync(status = emptyStatus): CloudSyncContextValue {
  return {
    cloudWorkspaceDialogOpen: false, detailTarget: null, enableTarget: null, available: true, hasCloudSyncCapability: true, errorCode: null,
    globalEnabled: true, loading: false, revision: 0, statuses: new Map([[workspace.id, status]]),
    closeCloudWorkspaceDialog: vi.fn(), closeDetailDialog: vi.fn(), closeEnableDialog: vi.fn(), enableWorkspace: vi.fn(),
    openCloudWorkspaceDialog: vi.fn(), openDetailDialog: vi.fn(), openEnableDialog: vi.fn(), pauseWorkspace: vi.fn(),
    refresh: vi.fn(), refreshNow: vi.fn(), replaceDeadLetterWithRemote: vi.fn(), retryDeadLetter: vi.fn(), retryWorkspace: vi.fn(), setServiceEnabled: vi.fn(),
  };
}

const t = ((key: string) => key) as Parameters<typeof createCloudSyncWorkspaceActions>[1];

describe("workspace Cloud Sync actions", () => {
  it("opens confirmation before enabling a local-only workspace", async () => {
    const context = sync();
    const [action] = createCloudSyncWorkspaceActions(context, t, workspace);
    await action.run({ workspace, activeWorkspace: workspace, activeTab: { id: "api", kind: "api", title: "API" }, activateWorkspace: vi.fn(), refreshWorkspaces: vi.fn() });
    expect(context.openEnableDialog).toHaveBeenCalledWith({ id: "workspace", name: "Backend" });
    expect(context.enableWorkspace).not.toHaveBeenCalled();
  });

  it("offers view and pause for an enabled workspace", () => {
    const context = sync({ ...emptyStatus, binding: { accountId: "a", localWorkspaceId: "workspace", cloudWorkspaceId: "c", lastPulledCursor: 0, syncEnabled: true, state: "active", initialCursor: 0, initialTotal: 0, initialConfirmed: 0, initializationCheckpoint: null, sshTaskV3BootstrapState: "completed", connectionV4BootstrapState: "completed", generation: 0, lastSuccessAt: null, lastError: null, consecutiveFailureCount: 0 } });
    expect(createCloudSyncWorkspaceActions(context, t, workspace).map((action) => action.label)).toEqual(["cloudSync.viewSyncStatus", "cloudSync.pauseCloudSync"]);
  });

  it("disables first-time enable when Cloud Sync is unavailable", () => {
    const context = { ...sync(), available: false, hasCloudSyncCapability: false };
    const [action] = createCloudSyncWorkspaceActions(context, t, workspace);
    expect(action.disabled).toBe(true);
    expect(action.disabledReason).toBe("cloudSync.capabilityDescription");
  });

  it("disables first-time enable when the paid account sync context failed", () => {
    const context = { ...sync(), available: false, errorCode: "cloud_sync_storage_failed" };
    const [action] = createCloudSyncWorkspaceActions(context, t, workspace);
    expect(action.disabled).toBe(true);
    expect(action.disabledReason).toBe("cloudSync.contextUnavailableDescription");
  });
});
