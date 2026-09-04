import type { CloudSyncStatus, CloudSyncViewState } from "./syncTypes";

const OFFLINE_ERRORS = new Set([
  "cloud_sync_transport_failed",
  "cloud_sync_temporarily_unavailable",
  "cloud_sync_timeout",
]);

const AUTH_ERRORS = new Set([
  "unauthorized",
  "cloud_sync_unauthorized",
  "cloud_sync_not_authenticated",
]);

const CAPABILITY_ERRORS = new Set([
  "cloud_sync_entitlement_required",
]);

export function getCloudSyncViewState(
  status: CloudSyncStatus,
  globalEnabled: boolean,
): CloudSyncViewState {
  const binding = status.binding;
  if (!binding) return "local_only";
  if (!globalEnabled || !binding.syncEnabled || binding.state === "paused") return "paused";
  if (status.conflictCount > 0 || binding.state === "conflict") return "attention";
  if (status.deadCount > 0) return "attention";
  const errorState = getErrorViewState(binding.lastError);
  if (errorState) return errorState;
  if (binding.state === "error") return "attention";
  if (binding.lastError) return "attention";
  return syncInProgress(status, binding) ? "syncing" : "synced";
}

function getErrorViewState(error: string | null): CloudSyncViewState | null {
  if (!error) return null;
  if (AUTH_ERRORS.has(error)) return "auth_required";
  if (CAPABILITY_ERRORS.has(error)) return "capability_required";
  if (OFFLINE_ERRORS.has(error)) return "offline";
  return null;
}

function syncInProgress(status: CloudSyncStatus, binding: NonNullable<CloudSyncStatus["binding"]>) {
  return status.running
    || binding.initialConfirmed < binding.initialTotal
    || ["preparing", "uploading", "downloading", "reconciling"].includes(binding.state)
    || status.pendingCount + status.uncertainCount + status.inFlightCount > 0;
}

export function viewStateTone(state: CloudSyncViewState): "neutral" | "success" | "warning" | "danger" {
  if (state === "synced") return "success";
  if (state === "attention") return "danger";
  if (state === "auth_required") return "danger";
  if (state === "capability_required") return "warning";
  if (["syncing", "offline"].includes(state)) return "warning";
  return "neutral";
}

export function syncErrorMessageKey(code: string): string {
  if (["cloud_sync_transport_failed", "cloud_sync_temporarily_unavailable", "cloud_sync_timeout"].includes(code)) {
    return "cloudSync.errors.network";
  }
  if (code === "cloud_sync_protocol_incompatible") return "cloudSync.errors.protocol";
  if (code === "cloud_sync_context_unavailable") return "cloudSync.errors.context";
  if (code === "cloud_sync_entitlement_required") return "cloudSync.errors.capability";
  if (["cloud_sync_unauthorized", "cloud_sync_not_authenticated"].includes(code)) return "cloudSync.errors.relogin";
  if (code === "account_not_signed_in") return "cloudSync.errors.signedOut";
  if (code === "cloud_sync_workspace_name_conflict") return "cloudSync.errors.workspaceNameConflict";
  if (code === "cloud_sync_safe_replace_unavailable") return "cloudSync.errors.safeReplaceUnavailable";
  return "cloudSync.errors.generic";
}
