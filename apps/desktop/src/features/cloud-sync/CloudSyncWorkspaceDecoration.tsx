import type { DesktopAppWorkspaceDecorationProps } from "@unfour/app-shell";
import { useI18n } from "@unfour/ui";
import { CloudSyncIcon } from "./CloudSyncIcon";
import { getCloudSyncViewState } from "./syncViewModel";
import { useCloudSync } from "./useCloudSync";

export function CloudSyncWorkspaceDecoration({ workspace }: DesktopAppWorkspaceDecorationProps) {
  const { t } = useI18n();
  const { globalEnabled, openDetailDialog, statuses } = useCloudSync();
  const status = statuses.get(workspace.id);
  if (!status?.binding) return null;
  const state = getCloudSyncViewState(status, globalEnabled);
  const label = t(`cloudSync.status.${state}`);
  return (
    <span
      aria-label={label}
      className="inline-flex shrink-0 cursor-pointer rounded p-0.5"
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        openDetailDialog({ id: workspace.id, name: workspace.name });
      }}
      onPointerDown={(event) => event.stopPropagation()}
      role="button"
      tabIndex={0}
      title={label}
    >
      <CloudSyncIcon state={state} />
    </span>
  );
}
