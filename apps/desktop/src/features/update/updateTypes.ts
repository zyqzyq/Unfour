export interface UpdateMeta {
  name: string;
  version: string;
  distribution: "standard" | "microsoft-store";
  channel: "test" | "stable";
  commit: string | null;
  updaterEnabled: boolean;
  endpoint: string | null;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  date: string | null;
  body: string | null;
}

export type UpdateDownloadEvent =
  | { event: "started"; contentLength: number | null }
  | { event: "progress"; chunkLength: number }
  | { event: "downloaded" }
  | { event: "installing" };

export type UpdateRecovery = "check" | "download" | "signature" | "installer";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "managedByStore" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; info: UpdateInfo }
  | { kind: "downloading"; info: UpdateInfo; downloaded: number; total: number | null }
  | { kind: "installing"; info: UpdateInfo }
  | { kind: "error"; message: string; info?: UpdateInfo; recovery: UpdateRecovery };

export interface UpdateContextValue {
  meta: UpdateMeta | null;
  state: UpdateState;
  dialogOpen: boolean;
  setDialogOpen(open: boolean): void;
  openDialog(): void;
  check(): Promise<void>;
  install(): Promise<void>;
}
