import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { TelemetryPreferences } from "@unfour/command-client";
import {
  getTelemetryPreferences,
  markTelemetryNoticeShown,
  recordTelemetryActive,
  setTelemetryEnabled,
} from "./telemetryApi";
import { TelemetryContext } from "./useTelemetry";
import { FIRST_NOTICE_GRACE_PERIOD_MS } from "./telemetryPolicy";

export function TelemetryProvider({ children }: { children: ReactNode }) {
  const [preferences, setPreferences] = useState<TelemetryPreferences | null>(null);
  const [noticeVisible, setNoticeVisible] = useState(false);
  const [preferenceError, setPreferenceError] = useState(false);
  const [updating, setUpdating] = useState(false);
  const graceDeadlineRef = useRef<number | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    getTelemetryPreferences()
      .then((initial) => {
        if (!mountedRef.current) return;
        if (!initial.noticeShown) {
          graceDeadlineRef.current = Date.now() + FIRST_NOTICE_GRACE_PERIOD_MS;
          setNoticeVisible(true);
          void markTelemetryNoticeShown()
            .then((updated) => {
              if (mountedRef.current) setPreferences(updated);
            })
            .catch(() => undefined);
        }
        setPreferences(initial);
      })
      .catch(() => {
        if (mountedRef.current) setPreferenceError(true);
      });
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (!preferences?.enabled) return undefined;
    const delay = Math.max(0, (graceDeadlineRef.current ?? Date.now()) - Date.now());
    const timer = window.setTimeout(() => {
      void recordTelemetryActive().catch(() => undefined);
    }, delay);
    return () => window.clearTimeout(timer);
  }, [preferences?.enabled]);

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      if (!preferences || updating || preferences.enabled === enabled) return;
      const previous = preferences;
      setPreferenceError(false);
      setUpdating(true);
      setPreferences({ ...previous, enabled });
      if (!enabled) setNoticeVisible(false);
      try {
        const updated = await setTelemetryEnabled(enabled);
        if (mountedRef.current) setPreferences(updated);
      } catch {
        if (mountedRef.current) {
          setPreferences(previous);
          setPreferenceError(true);
        }
      } finally {
        if (mountedRef.current) setUpdating(false);
      }
    },
    [preferences, updating],
  );

  const value = useMemo(
    () => ({
      dismissNotice: () => setNoticeVisible(false),
      noticeVisible,
      preferenceError,
      preferences,
      setEnabled,
      updating,
    }),
    [noticeVisible, preferenceError, preferences, setEnabled, updating],
  );

  return <TelemetryContext.Provider value={value}>{children}</TelemetryContext.Provider>;
}
