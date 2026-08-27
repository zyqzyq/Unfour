import type { DesktopAppWorkspaceAction } from "@unfour/app-shell";
import type { TFunction } from "@unfour/ui";
import { getCloudSyncViewState } from "./syncViewModel";
import type { CloudSyncContextValue } from "./useCloudSync";

export function createCloudSyncWorkspaceActions(
  sync: CloudSyncContextValue,
  t: TFunction,
  workspace: Readonly<{ id: string; name: string }>,
): DesktopAppWorkspaceAction[] {
  const status = sync.statuses.get(workspace.id);
  if (!status?.binding) return [{
    id: "cloud-sync.enable-workspace",
    label: t("cloudSync.enableCloudSync"),
    disabled: !sync.available,
    disabledReason: t(sync.hasCloudSyncCapability
      ? "cloudSync.contextUnavailableDescription"
      : "cloudSync.capabilityDescription"),
    run: () => sync.openEnableDialog({ id: workspace.id, name: workspace.name }),
  }];

  const state = getCloudSyncViewState(status, sync.globalEnabled);
  const actions: DesktopAppWorkspaceAction[] = [{
    id: "cloud-sync.view-status",
    label: t("cloudSync.viewSyncStatus"),
    run: () => sync.openDetailDialog({ id: workspace.id, name: workspace.name }),
  }, status.binding.syncEnabled ? {
    id: "cloud-sync.pause-workspace",
    label: t("cloudSync.pauseCloudSync"),
    run: () => sync.pauseWorkspace(workspace.id),
  } : {
    id: "cloud-sync.resume-workspace",
    label: t("cloudSync.resumeCloudSync"),
    run: () => sync.enableWorkspace(workspace.id),
  }];
  if (state === "attention") actions.push({
    id: "cloud-sync.resolve-issue",
    label: t("cloudSync.resolveSyncIssue"),
    run: () => sync.openDetailDialog({ id: workspace.id, name: workspace.name }),
  });
  return actions;
}
