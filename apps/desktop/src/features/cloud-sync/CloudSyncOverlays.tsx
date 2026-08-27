import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import { CloudWorkspaceDialog } from "./CloudWorkspaceDialog";
import { EnableCloudSyncDialog } from "./EnableCloudSyncDialog";
import { WorkspaceSyncDialog } from "./WorkspaceSyncDialog";

export function CloudSyncOverlays(context: DesktopAppExtensionContext) {
  return <><EnableCloudSyncDialog /><CloudWorkspaceDialog {...context} /><WorkspaceSyncDialog /></>;
}
