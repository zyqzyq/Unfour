export type SystemHealth = {
  appName: string;
  storageReady: boolean;
  commandBusReady: boolean;
  aiReservedCapabilities: string[];
};

export type DiagnosticBundleResult = {
  bundleDir: string;
  manifestPath: string;
};

export type AppDistribution = "standard" | "microsoft-store";

export type AppChannel = "test" | "stable";

export type AppInfo = {
  name: string;
  version: string;
  distribution: AppDistribution;
  channel: AppChannel;
  commit: string | null;
};

export type TelemetryPreferences = {
  enabled: boolean;
  noticeShown: boolean;
  networkEnabled: boolean;
};

export type TelemetrySendOutcome =
  | "sent"
  | "alreadySentToday"
  | "disabled"
  | "networkDisabled"
  | "failed";
