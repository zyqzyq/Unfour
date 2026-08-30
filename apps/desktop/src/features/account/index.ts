import { createElement } from "react";
import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { AccountSection, AccountSectionLabel } from "./AccountSection";

export { AccountProvider } from "./AccountProvider";
export { AccountIndicator } from "./AccountIndicator";
export { AccountOverlays } from "./AccountOverlays";
export { AccountPlanSummary } from "./AccountPlanSummary";
export { AccountSection } from "./AccountSection";
export { accountI18nResources } from "./locales";
export { useAccount } from "./useAccount";
export {
  CLOUD_SYNC_ENTITLEMENT,
  TEAM_WORKSPACE_ENTITLEMENT,
  hasActiveEntitlement,
} from "./accountEntitlement";
export type {
  AccountMockState,
  AccountProfile,
  AccountState,
  EntitlementSummary,
} from "./accountTypes";

export const accountSection: DesktopAppSettingsSection = {
  id: "account.settings",
  label: createElement(AccountSectionLabel),
  component: AccountSection,
};
