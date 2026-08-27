export type EntitlementStatus = "active" | "expired" | "revoked" | "suspended";

export interface EntitlementSummary {
  code: string;
  status: EntitlementStatus;
  validUntil: string | null;
}

export interface DeviceSummary {
  id: string;
  name: string;
  platform: string;
  lastSeenAt: string | null;
  revoked: boolean;
}

export interface AccountProfile {
  id: string;
  email: string;
  username: string | null;
  displayName: string | null;
  avatarUrl: string | null;
  entitlements: EntitlementSummary[];
  devices: DeviceSummary[];
}

export type AccountMockState = "signedOut" | "signingIn" | "signedIn" | "error";

export type AccountState =
  | { kind: "signedOut" }
  | { kind: "signingIn" }
  | { kind: "signedIn"; profile: AccountProfile }
  | { kind: "error" };

export type CloudSyncAccountContextState =
  | { kind: "ready" }
  | { kind: "inactive" }
  | { kind: "error"; code: string };

export interface AccountStateSnapshot {
  account: AccountState;
  syncContext: CloudSyncAccountContextState;
}

export interface AccountContextValue {
  preview: boolean;
  state: AccountState;
  syncContext: CloudSyncAccountContextState;
  overlayOpen: boolean;
  setOverlayOpen(open: boolean): void;
  openOverlay(): void;
  signIn(): void;
  signOut(): void;
  retry(): void;
  refreshAccount(): Promise<void>;
  refreshing: boolean;
  setMockState(state: AccountMockState): void;
}
