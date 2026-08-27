import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import { IconButton, useI18n } from "@unfour/ui";
import { CloudSyncIcon } from "./CloudSyncIcon";
import { getCloudSyncViewState } from "./syncViewModel";
import { useCloudSync } from "./useCloudSync";

export function CloudSyncStatus({ activeWorkspace }: DesktopAppExtensionContext) {
  const { t } = useI18n();
  const { globalEnabled, openDetailDialog, statuses } = useCloudSync();
  if (!activeWorkspace) return null;
  const status = statuses.get(activeWorkspace.id);
  if (!status?.binding) return null;
  const viewState = getCloudSyncViewState(status, globalEnabled);
  const label = t(`cloudSync.status.${viewState}`);
  return <IconButton label={label} onClick={() => openDetailDialog({ id: activeWorkspace.id, name: activeWorkspace.name })} size="compact"><CloudSyncIcon size={15} state={viewState} /></IconButton>;
}
