import { createElement } from "react";
import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { ApiClientSettings } from "@unfour/api-client";
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

const apiClientSettingsSection: DesktopAppSettingsSection = {
  id: "api-client.settings",
  component: ApiClientSettings,
  slot: "general",
};

export const desktopSettingsSections: readonly DesktopAppSettingsSection[] = [
  apiClientSettingsSection,
  telemetryPrivacySection,
  accountSyncSection,
  updatesSection,
];
