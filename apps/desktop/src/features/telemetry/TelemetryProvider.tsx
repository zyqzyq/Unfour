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
  const [sessionTelemetrySuppressed, setSessionTelemetrySuppressed] = useState(false);
  const graceDeadlineRef = useRef<number | null>(null);
  const pendingTimerRef = useRef<number | null>(null);
  const sessionTelemetrySuppressedRef = useRef(false);
  const noticeMarkAttemptedRef = useRef(false);
  const mountedRef = useRef(false);

  const cancelPendingTimer = useCallback(() => {
    if (pendingTimerRef.current === null) return;
    window.clearTimeout(pendingTimerRef.current);
    pendingTimerRef.current = null;
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    getTelemetryPreferences()
      .then((initial) => {
        if (!mountedRef.current) return;
        if (!initial.noticeShown && initial.networkEnabled) {
          graceDeadlineRef.current = Date.now() + FIRST_NOTICE_GRACE_PERIOD_MS;
          setNoticeVisible(true);
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
    if (
      !preferences?.enabled
      || !preferences.networkEnabled
      || sessionTelemetrySuppressed
    ) {
      cancelPendingTimer();
      return undefined;
    }
    const delay = Math.max(0, (graceDeadlineRef.current ?? Date.now()) - Date.now());
    const timer = window.setTimeout(() => {
      pendingTimerRef.current = null;
      if (sessionTelemetrySuppressedRef.current) return;
      void recordTelemetryActive().catch(() => undefined);
    }, delay);
    pendingTimerRef.current = timer;
    return () => {
      window.clearTimeout(timer);
      if (pendingTimerRef.current === timer) pendingTimerRef.current = null;
    };
  }, [cancelPendingTimer, preferences?.enabled, preferences?.networkEnabled, sessionTelemetrySuppressed]);

  const markNoticeShown = useCallback(async () => {
    if (noticeMarkAttemptedRef.current) return;
    noticeMarkAttemptedRef.current = true;
    try {
      const updated = await markTelemetryNoticeShown();
      if (mountedRef.current) {
        setPreferences((current) =>
          current ? { ...current, noticeShown: updated.noticeShown } : updated,
        );
      }
    } catch {
      // Notice persistence is best effort and must not affect app usage.
    }
  }, []);

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      if (
        !preferences
        || updating
        || (preferences.enabled === enabled
          && !(enabled && sessionTelemetrySuppressedRef.current))
      ) return;
      const previous = preferences;
      setPreferenceError(false);
      setUpdating(true);
      if (!enabled) {
        sessionTelemetrySuppressedRef.current = true;
        setSessionTelemetrySuppressed(true);
        cancelPendingTimer();
        setNoticeVisible(false);
      } else {
        sessionTelemetrySuppressedRef.current = false;
        setSessionTelemetrySuppressed(false);
      }
      setPreferences({ ...previous, enabled });
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
    [cancelPendingTimer, preferences, updating],
  );

  const value = useMemo(
    () => ({
      dismissNotice: () => setNoticeVisible(false),
      markNoticeShown,
      noticeVisible,
      preferenceError,
      preferences,
      setEnabled,
      updating,
    }),
    [markNoticeShown, noticeVisible, preferenceError, preferences, setEnabled, updating],
  );

  return <TelemetryContext.Provider value={value}>{children}</TelemetryContext.Provider>;
}
