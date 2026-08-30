import { createElement } from "react";
import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { CloudSyncSection, CloudSyncSectionLabel } from "./CloudSyncSection";

export { CloudSyncProvider } from "./CloudSyncProvider";
export { CloudSyncOverlays } from "./CloudSyncOverlays";
export { CloudSyncStatus } from "./CloudSyncStatus";
export { CloudSyncWorkspaceDecoration } from "./CloudSyncWorkspaceDecoration";
export { CloudSyncSection } from "./CloudSyncSection";
export { cloudSyncI18nResources } from "./locales";
export { useCloudSync } from "./useCloudSync";

export const cloudSyncSection: DesktopAppSettingsSection = {
  id: "cloud-sync.settings",
  label: createElement(CloudSyncSectionLabel),
  component: CloudSyncSection,
};
