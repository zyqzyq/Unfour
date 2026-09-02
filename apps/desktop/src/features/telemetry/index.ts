import type { DesktopAppSettingsSection } from "@unfour/app-shell";
import { PrivacySection } from "./PrivacySection";

export { PrivacySection } from "./PrivacySection";
export { TelemetryNotice } from "./TelemetryNotice";
export { TelemetryProvider } from "./TelemetryProvider";
export { telemetryI18nResources } from "./locales";
export { useTelemetry } from "./useTelemetry";

export const telemetryPrivacySection: DesktopAppSettingsSection = {
  id: "telemetry.privacy",
  component: PrivacySection,
  slot: "general",
};
