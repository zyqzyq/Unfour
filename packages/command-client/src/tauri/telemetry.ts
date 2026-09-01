import type {
  TelemetryPreferences,
  TelemetrySendOutcome,
} from "../types";
import { call } from "./invoke";

export function getTelemetryPreferences() {
  return call<TelemetryPreferences>("telemetry_get_preferences");
}

export function setTelemetryEnabled(enabled: boolean) {
  return call<TelemetryPreferences>("telemetry_set_enabled", { enabled });
}

export function markTelemetryNoticeShown() {
  return call<TelemetryPreferences>("telemetry_mark_notice_shown");
}

export function recordTelemetryActive() {
  return call<TelemetrySendOutcome>("telemetry_record_active");
}
