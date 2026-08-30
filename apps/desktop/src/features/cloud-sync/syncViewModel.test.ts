import { describe, expect, it } from "vitest";
import type { CloudSyncStatus, SyncBinding } from "./syncTypes";
import { getCloudSyncViewState, syncErrorMessageKey } from "./syncViewModel";

const binding: SyncBinding = {
  accountId: "account",
  localWorkspaceId: "workspace",
  cloudWorkspaceId: "cloud",
  lastPulledCursor: 1,
  syncEnabled: true,
  state: "active",
  initialCursor: 0,
  initialTotal: 1,
  initialConfirmed: 1,
  initializationCheckpoint: null,
  sshTaskV3BootstrapState: "completed",
  connectionV4BootstrapState: "completed",
  generation: 1,
  lastSuccessAt: "2026-08-08T00:00:00.000Z",
  lastError: null,
  consecutiveFailureCount: 0,
};

function status(overrides: Partial<CloudSyncStatus> = {}, bindingOverrides: Partial<SyncBinding> = {}): CloudSyncStatus {
  return { binding: { ...binding, ...bindingOverrides }, pendingCount: 0, uncertainCount: 0, inFlightCount: 0, deadCount: 0, deadLetters: [], conflictCount: 0, running: false, ...overrides };
}

describe("getCloudSyncViewState", () => {
  it("keeps pause and error states ahead of ongoing synchronization", () => {
    expect(getCloudSyncViewState(status({ running: true }), false)).toBe("paused");
    expect(getCloudSyncViewState(status({ running: true, deadCount: 1 }), true)).toBe("attention");
    expect(getCloudSyncViewState(status({ running: true }, { lastError: "cloud_sync_timeout" }), true)).toBe("offline");
    expect(getCloudSyncViewState(status({}, { initialConfirmed: 0, initialTotal: 2 }), true)).toBe("syncing");
    expect(getCloudSyncViewState(status({ uncertainCount: 1 }), true)).toBe("syncing");
  });

  it("maps local-only, syncing, synced, and paused workspaces", () => {
    expect(getCloudSyncViewState({ ...status(), binding: null }, true)).toBe("local_only");
    expect(getCloudSyncViewState(status({ pendingCount: 1 }), true)).toBe("syncing");
    expect(getCloudSyncViewState(status(), true)).toBe("synced");
    expect(getCloudSyncViewState(status(), false)).toBe("paused");
    expect(getCloudSyncViewState(status({}, { syncEnabled: false }), true)).toBe("paused");
  });

  it("maps temporary connectivity failures to offline", () => {
    expect(getCloudSyncViewState(status({}, {
      state: "error",
      lastError: "cloud_sync_transport_failed",
    }), true)).toBe("offline");
  });

  it("maps conflicts, dead letters, and permanent errors to attention", () => {
    expect(getCloudSyncViewState(status({ conflictCount: 1 }), true)).toBe("attention");
    expect(getCloudSyncViewState(status({ deadCount: 1 }), true)).toBe("attention");
    expect(getCloudSyncViewState(status({}, { state: "error" }), true)).toBe("attention");
  });

  it("keeps user-facing error categories distinct", () => {
    expect(syncErrorMessageKey("cloud_sync_transport_failed")).toBe("cloudSync.errors.network");
    expect(syncErrorMessageKey("cloud_sync_protocol_incompatible")).toBe("cloudSync.errors.protocol");
    expect(syncErrorMessageKey("cloud_sync_context_unavailable")).toBe("cloudSync.errors.context");
    expect(syncErrorMessageKey("cloud_sync_entitlement_required")).toBe("cloudSync.errors.capability");
    expect(syncErrorMessageKey("cloud_sync_unauthorized")).toBe("cloudSync.errors.relogin");
    expect(syncErrorMessageKey("cloud_sync_not_authenticated")).toBe("cloudSync.errors.relogin");
    expect(syncErrorMessageKey("cloud_sync_safe_replace_unavailable")).toBe("cloudSync.errors.safeReplaceUnavailable");
    expect(syncErrorMessageKey("cloud_sync_storage_failed")).toBe("cloudSync.errors.generic");
  });
});
