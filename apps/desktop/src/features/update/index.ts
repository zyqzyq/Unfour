import { createElement } from "react";
import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { UpdatesSection, UpdatesSectionLabel } from "./UpdatesSection";

export { UpdateProvider } from "./UpdateProvider";
export { UpdateIndicator } from "./UpdateIndicator";
export { UpdateOverlays } from "./UpdateDialog";
export { UpdatesSection } from "./UpdatesSection";
export { updateI18nResources } from "./locales";
export { useUpdate } from "./useUpdate";
export type { UpdateInfo, UpdateMeta, UpdateState } from "./updateTypes";

export const updatesSection: DesktopAppSettingsSection = {
  id: "updates.settings",
  label: createElement(UpdatesSectionLabel),
  component: UpdatesSection,
};
