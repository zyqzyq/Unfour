// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type {
  AccountContextValue,
  AccountProfile,
  EntitlementStatus,
} from "../account/accountTypes";

const mocks = vi.hoisted(() => ({
  account: null as unknown as AccountContextValue,
  disableCloudSync: vi.fn(),
  enableCloudSync: vi.fn(),
  getCloudSyncStatus: vi.fn(),
  getGlobalSyncEnabled: vi.fn(),
  getLocalWorkspaces: vi.fn(),
  retryDeadLetterCurrentLocal: vi.fn(),
  setGlobalSyncEnabled: vi.fn(),
  syncNow: vi.fn(),
  replaceDeadLetterWithRemote: vi.fn(),
}));

vi.mock("../account/useAccount", () => ({ useAccount: () => mocks.account }));
vi.mock("./syncApi", () => ({
  disableCloudSync: mocks.disableCloudSync,
  enableCloudSync: mocks.enableCloudSync,
  getCloudSyncStatus: mocks.getCloudSyncStatus,
  getGlobalSyncEnabled: mocks.getGlobalSyncEnabled,
  getLocalWorkspaces: mocks.getLocalWorkspaces,
  retryDeadLetterCurrentLocal: mocks.retryDeadLetterCurrentLocal,
  setGlobalSyncEnabled: mocks.setGlobalSyncEnabled,
  syncErrorCode: (error: { code?: string }) => error.code ?? "cloud_sync_failed",
  syncNow: mocks.syncNow,
  replaceDeadLetterWithRemote: mocks.replaceDeadLetterWithRemote,
}));

import { CloudSyncProvider } from "./CloudSyncProvider";
import { useCloudSync } from "./useCloudSync";

function profile(
  hasCloudSyncCapability: boolean,
  status: EntitlementStatus = "active",
): AccountProfile {
  return {
    id: "account-a",
    email: "account-a@example.test",
    username: "account-a",
    displayName: "Account A",
    avatarUrl: null,
    entitlements: hasCloudSyncCapability
      ? [{ code: "cloud_sync", status, validUntil: null }]
      : [],
    devices: [],
  };
}

function account(
  hasCloudSyncCapability: boolean,
  syncContext: AccountContextValue["syncContext"],
  status: EntitlementStatus = "active",
): AccountContextValue {
  return {
    preview: false,
    state: { kind: "signedIn", profile: profile(hasCloudSyncCapability, status) },
    syncContext,
    overlayOpen: false,
    refreshing: false,
    openOverlay: vi.fn(),
    refreshAccount: vi.fn(),
    retry: vi.fn(),
    setMockState: vi.fn(),
    setOverlayOpen: vi.fn(),
    signIn: vi.fn(),
    signOut: vi.fn(),
  };
}

function anonymousAccount(): AccountContextValue {
  return {
    ...account(false, { kind: "inactive" }),
    state: { kind: "signedOut" },
  };
}

function Probe() {
  const sync = useCloudSync();
  return <div>
    <span data-testid="capability">{String(sync.hasCloudSyncCapability)}</span>
    <span data-testid="available">{String(sync.available)}</span>
    <span data-testid="error">{sync.errorCode ?? "none"}</span>
    <span data-testid="statuses">{sync.statuses.size}</span>
    <span data-testid="binding-enabled">
      {String(sync.statuses.get("workspace")?.binding?.syncEnabled ?? false)}
    </span>
    <button onClick={() => void sync.enableWorkspace("workspace").catch(() => undefined)} type="button">enable</button>
  </div>;
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.getLocalWorkspaces.mockResolvedValue({
    activeWorkspaceId: "workspace",
    workspaces: [{ id: "workspace", name: "Workspace" }],
  });
  mocks.getGlobalSyncEnabled.mockResolvedValue(true);
  mocks.getCloudSyncStatus.mockResolvedValue({
    binding: null,
    pendingCount: 0,
    uncertainCount: 0,
    inFlightCount: 0,
    deadCount: 0,
    deadLetters: [],
    conflictCount: 0,
    running: false,
  });
  mocks.enableCloudSync.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("CloudSyncProvider account context boundary", () => {
  it.each([
    ["anonymous", anonymousAccount()],
    ["logged-in free", account(false, { kind: "inactive" })],
    ["expired entitlement", account(true, { kind: "inactive" }, "expired")],
  ])("keeps %s users local-only", async (_label, accountValue) => {
    mocks.account = accountValue;
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    expect(screen.getByTestId("capability")).toHaveTextContent("false");
    expect(screen.getByTestId("available")).toHaveTextContent("false");
    fireEvent.click(screen.getByRole("button", { name: "enable" }));
    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_entitlement_required"));
    expect(mocks.enableCloudSync).not.toHaveBeenCalled();
    expect(mocks.getLocalWorkspaces).not.toHaveBeenCalled();
  });

  it("keeps the Cloud Sync capability visible but reports sync unavailable after activation failure", async () => {
    mocks.account = account(true, { kind: "error", code: "cloud_sync_storage_failed" });
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    expect(screen.getByTestId("capability")).toHaveTextContent("true");
    expect(screen.getByTestId("available")).toHaveTextContent("false");
    expect(screen.getByTestId("error")).toHaveTextContent("cloud_sync_storage_failed");
    fireEvent.click(screen.getByRole("button", { name: "enable" }));
    await Promise.resolve();
    expect(mocks.enableCloudSync).not.toHaveBeenCalled();
    expect(mocks.getLocalWorkspaces).not.toHaveBeenCalled();
  });

  it("keeps a revoked account Free and blocks sync after local cleanup failure", async () => {
    mocks.account = account(false, { kind: "error", code: "cloud_sync_storage_failed" });
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    expect(screen.getByTestId("capability")).toHaveTextContent("false");
    expect(screen.getByTestId("available")).toHaveTextContent("false");
    expect(screen.getByTestId("error")).toHaveTextContent("cloud_sync_storage_failed");
    fireEvent.click(screen.getByRole("button", { name: "enable" }));
    await Promise.resolve();
    expect(mocks.enableCloudSync).not.toHaveBeenCalled();
  });

  it("blocks new sync requests for a suspended signed-in account", async () => {
    mocks.account = account(true, { kind: "inactive" }, "suspended");
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    expect(screen.getByTestId("capability")).toHaveTextContent("false");
    expect(screen.getByTestId("available")).toHaveTextContent("false");
    fireEvent.click(screen.getByRole("button", { name: "enable" }));
    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_entitlement_required"));
    expect(mocks.enableCloudSync).not.toHaveBeenCalled();
    expect(mocks.getLocalWorkspaces).not.toHaveBeenCalled();
  });

  it("preserves normal activation and status loading", async () => {
    mocks.account = account(true, { kind: "ready" });
    mocks.getCloudSyncStatus.mockResolvedValue({
      binding: {
        accountId: "account-a",
        localWorkspaceId: "workspace",
        cloudWorkspaceId: "cloud-workspace",
        lastPulledCursor: 1,
        syncEnabled: true,
        state: "active",
        initialCursor: 0,
        initialTotal: 1,
        initialConfirmed: 1,
        initializationCheckpoint: null,
        sshTaskV3BootstrapState: "completed",
        connectionV4BootstrapState: "completed",
        generation: 1,
        lastSuccessAt: "2026-08-27T00:00:00.000Z",
        lastError: null,
        consecutiveFailureCount: 0,
      },
      pendingCount: 0,
      uncertainCount: 0,
      inFlightCount: 0,
      deadCount: 0,
      deadLetters: [],
      conflictCount: 0,
      running: false,
    });
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    expect(screen.getByTestId("binding-enabled")).toHaveTextContent("true");
    expect(screen.getByTestId("available")).toHaveTextContent("true");
    expect(screen.getByTestId("error")).toHaveTextContent("none");
    expect(mocks.getLocalWorkspaces).toHaveBeenCalledTimes(1);
    expect(mocks.getGlobalSyncEnabled).toHaveBeenCalledTimes(1);
  });
});
