export type SystemHealth = {
  appName: string;
  storageReady: boolean;
  commandBusReady: boolean;
  aiReservedCapabilities: string[];
  syncStrategy: string;
};

export type DiagnosticBundleResult = {
  bundleDir: string;
  manifestPath: string;
};

export type AppEdition = "community" | "pro";

export type AppInfo = {
  version: string;
  edition: AppEdition;
};
