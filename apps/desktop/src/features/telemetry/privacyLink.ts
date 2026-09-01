import { openUrl } from "@tauri-apps/plugin-opener";

export const TELEMETRY_PRIVACY_URL =
  "https://github.com/zyqzyq/Unfour/blob/main/docs/privacy/telemetry.md";

export function openTelemetryPrivacy() {
  return openUrl(TELEMETRY_PRIVACY_URL);
}
