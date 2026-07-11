export type SshAuthKind = "password" | "private-key" | "none";

export type SshConnectionInput = {
  id?: string;
  workspaceId: string;
  name: string;
  host: string;
  port?: number | null;
  username: string;
  authKind: SshAuthKind;
  keyPath?: string | null;
  credentialRef?: string | null;
  /** Plaintext password / key passphrase; stored in the OS keychain on save,
   * never persisted to SQLite. Leave null when editing to keep the saved one. */
  secret?: string | null;
};

export type CredentialCreateInput = {
  workspaceId: string;
  kind: string;
  label: string;
  secret: string;
};

export type CredentialDeleteInput = {
  workspaceId: string;
  credentialRef: string;
};

export type CredentialInspectInput = {
  workspaceId: string;
  credentialRef: string;
};

export type CredentialRotateInput = {
  workspaceId: string;
  credentialRef: string;
  secret: string;
};

export type CredentialMetadata = {
  workspaceId: string;
  kind: string;
  label: string;
  credentialRef: string;
};

export type SshConnection = {
  id: string;
  workspaceId: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authKind: SshAuthKind;
  keyPath: string | null;
  credentialRef: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type SshTestResult = {
  ok: boolean;
  message: string;
};

export type SshConnectInput = {
  workspaceId: string;
  connectionId: string;
  cols?: number | null;
  rows?: number | null;
  /**
   * Transient credential override for validating a not-yet-saved secret (e.g.
   * the "test connection" action). When omitted, the saved keychain credential
   * is used. Never persisted.
   */
  secret?: string | null;
};

export type SshSessionInput = {
  workspaceId: string;
  sessionId: string;
  data: string;
};

export type SshResizeInput = {
  workspaceId: string;
  sessionId: string;
  cols: number;
  rows: number;
};

export type SshCloseInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshReconnectCancelInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshLogExportInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshSessionSummary = {
  sessionId: string;
  workspaceId: string;
  connectionId: string;
  status: "connected" | "degraded" | "reconnecting" | "disconnected" | "failed";
  reconnectAttempt: number;
  authKind: SshAuthKind;
  host: string;
  username: string;
  cols: number;
  rows: number;
  createdAt: string;
  updatedAt: string;
};

export type SshSessionEvent = {
  sessionId: string;
  kind: "input" | "output" | "resize" | "close";
  data: string;
  createdAt: string;
};

export type SshLogExport = {
  sessionId: string;
  filename: string;
  content: string;
  lineCount: number;
  redacted: boolean;
};

export type SshHostKeyInput = {
  workspaceId: string;
  host: string;
  port: number;
};

export type SshHostFingerprintInfo = {
  workspaceId: string;
  host: string;
  port: number;
  fingerprint: string;
  createdAt: string;
};

export type SshKnownHostsImportInput = {
  workspaceId: string;
  content: string;
};

export type SshKnownHostsExportInput = {
  workspaceId: string;
};

export type SshKnownHostsImportResult = {
  imported: number;
  skipped: number;
  errors: string[];
};

export type SshKnownHostsExportResult = {
  content: string;
  entryCount: number;
};
