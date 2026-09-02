import { createElement } from "react";
import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { useI18n } from "@unfour/ui";
import { AccountSyncSettings } from "./desktopSettings";
import { telemetryPrivacySection } from "./features/telemetry";
import { updatesSection } from "./features/update";

function AccountSyncSettingsLabel() {
  const { t } = useI18n();
  return t("app.settings.sections.accountSync");
}

const accountSyncSection: DesktopAppSettingsSection = {
  id: "account-sync.settings",
  label: createElement(AccountSyncSettingsLabel),
  component: AccountSyncSettings,
};

export const desktopSettingsSections: readonly DesktopAppSettingsSection[] = [
  telemetryPrivacySection,
  accountSyncSection,
  updatesSection,
];
