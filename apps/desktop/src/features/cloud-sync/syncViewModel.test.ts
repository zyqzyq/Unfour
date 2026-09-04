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

  it("maps authentication failures to a sign-in recovery state", () => {
    expect(getCloudSyncViewState(status({}, {
      state: "error",
      lastError: "cloud_sync_unauthorized",
    }), true)).toBe("auth_required");
    expect(getCloudSyncViewState(status({}, {
      state: "error",
      lastError: "unauthorized",
    }), true)).toBe("auth_required");
  });

  it("maps entitlement failures to a plan-required state", () => {
    expect(getCloudSyncViewState(status({}, {
      state: "error",
      lastError: "cloud_sync_entitlement_required",
    }), true)).toBe("capability_required");
  });

  it("maps conflicts, dead letters, and permanent errors to attention", () => {
    expect(getCloudSyncViewState(status({ conflictCount: 1 }), true)).toBe("attention");
    expect(getCloudSyncViewState(status({ deadCount: 1 }), true)).toBe("attention");
    expect(getCloudSyncViewState(status({}, { state: "error" }), true)).toBe("attention");
  });

  it("keeps user-facing error categories distinct", () => {
    expect(syncErrorMessageKey("cloud_sync_transport_failed")).toBe("cloudSync.errors.network");
    expect(syncErrorMessageKey("cloud_sync_server_unavailable")).toBe("cloudSync.errors.server");
    expect(syncErrorMessageKey("cloud_sync_protocol_incompatible")).toBe("cloudSync.errors.protocol");
    expect(syncErrorMessageKey("invalid_sync_entity")).toBe("cloudSync.errors.invalidData");
    expect(syncErrorMessageKey("cloud_sync_permanent_failure")).toBe("cloudSync.errors.requestRejected");
    expect(syncErrorMessageKey("invalid_request")).toBe("cloudSync.errors.requestRejected");
    expect(syncErrorMessageKey("cloud_sync_context_unavailable")).toBe("cloudSync.errors.context");
    expect(syncErrorMessageKey("cloud_sync_entitlement_required")).toBe("cloudSync.errors.capability");
    expect(syncErrorMessageKey("cloud_sync_unauthorized")).toBe("cloudSync.errors.relogin");
    expect(syncErrorMessageKey("cloud_sync_not_authenticated")).toBe("cloudSync.errors.relogin");
    expect(syncErrorMessageKey("cloud_sync_safe_replace_unavailable")).toBe("cloudSync.errors.safeReplaceUnavailable");
    const expected = new Map([
      ["cloud_sync_not_found", "cloudSync.errors.workspaceNotFound"],
      ["cloud_sync_storage_failed", "cloudSync.errors.storage"],
      ["cloud_sync_core_apply_failed", "cloudSync.errors.localApply"],
      ["cloud_sync_account_changed", "cloudSync.errors.accountChanged"],
      ["cloud_sync_conflict", "cloudSync.errors.conflict"],
      ["cloud_sync_local_workspace_not_empty", "cloudSync.errors.localWorkspaceNotEmpty"],
      ["cloud_sync_workspace_name_conflict", "cloudSync.errors.workspaceNameConflict"],
      ["cloud_sync_cloud_workspace_not_empty", "cloudSync.errors.cloudWorkspaceNotEmpty"],
      ["cloud_sync_dead_letter_blocked", "cloudSync.errors.deadLetterBlocked"],
      ["cloud_sync_workspace_owned_by_another_account", "cloudSync.errors.ownership"],
      ["cloud_sync_workspace_ownership_ambiguous", "cloudSync.errors.ownership"],
      ["cloud_sync_workspace_ownership_invariant", "cloudSync.errors.ownership"],
      ["cloud_sync_snapshot_required", "cloudSync.errors.snapshotRequired"],
      ["cloud_sync_workspace_deleted", "cloudSync.errors.workspaceDeleted"],
      ["invalid_parent_entity", "cloudSync.errors.invalidData"],
      ["payload_schema_version_unsupported", "cloudSync.errors.payloadSchema"],
      ["operation_id_reuse", "cloudSync.errors.operationIdReuse"],
      ["secret_value_not_allowed", "cloudSync.errors.secretRejected"],
      ["request_too_large", "cloudSync.errors.tooLarge"],
      ["payload_too_large", "cloudSync.errors.tooLarge"],
      ["protocol_version_unsupported", "cloudSync.errors.protocol"],
    ]);
    for (const [code, key] of expected) {
      expect(syncErrorMessageKey(code), code).toBe(key);
      expect(syncErrorMessageKey(code), `${code} must not be generic`).not.toBe("cloudSync.errors.generic");
    }
  });
});
