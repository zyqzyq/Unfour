import type { TelemetryPreferences } from "../../types";
import { UNHANDLED, type MockResult } from "./types";

let preferences: TelemetryPreferences = {
  enabled: true,
  noticeShown: false,
  networkEnabled: false,
};

export function handleTelemetryMock<T>(
  command: string,
  args?: Record<string, unknown>,
): MockResult<T> {
  if (command === "telemetry_get_preferences") {
    return { ...preferences } as T;
  }
  if (command === "telemetry_mark_notice_shown") {
    preferences = { ...preferences, noticeShown: true };
    return { ...preferences } as T;
  }
  if (command === "telemetry_set_enabled") {
    preferences = { ...preferences, enabled: args?.enabled === true };
    return { ...preferences } as T;
  }
  if (command === "telemetry_record_active") {
    return (preferences.enabled ? "networkDisabled" : "disabled") as T;
  }
  return UNHANDLED;
}
