import { createContext, useContext } from "react";
import type { TelemetryPreferences } from "@unfour/command-client";

export type TelemetryContextValue = {
  dismissNotice: () => void;
  markNoticeShown: () => Promise<void>;
  noticeVisible: boolean;
  preferenceError: boolean;
  preferences: TelemetryPreferences | null;
  setEnabled: (enabled: boolean) => Promise<void>;
  updating: boolean;
};

export const TelemetryContext = createContext<TelemetryContextValue | null>(null);

export function useTelemetry() {
  const context = useContext(TelemetryContext);
  if (!context) {
    throw new Error("useTelemetry must be used inside TelemetryProvider");
  }
  return context;
}
