// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type {
  AccountContextValue,
  AccountProfile,
  AccountStateSnapshot,
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
    <span data-testid="workspace-errors">{sync.workspaceErrors.size}</span>
    <span data-testid="binding-enabled">
      {String(sync.statuses.get("workspace")?.binding?.syncEnabled ?? false)}
    </span>
    <button onClick={() => void sync.enableWorkspace("workspace").catch(() => undefined)} type="button">enable</button>
    <button onClick={() => void sync.retryWorkspace("workspace").catch(() => undefined)} type="button">retry</button>
  </div>;
}

function accountSnapshot(
  accountValue: AccountContextValue,
  account = accountValue.state,
  syncContext = accountValue.syncContext,
): AccountStateSnapshot {
  return { account, syncContext };
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
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("CloudSyncProvider polling lifecycle", () => {
  it("polls once per interval, never syncs from rerenders, and stops after unmount", async () => {
    vi.useFakeTimers();
    mocks.account = account(true, { kind: "ready" });
    const { rerender, unmount } = render(<CloudSyncProvider><Probe /></CloudSyncProvider>);
    await act(async () => {});
    rerender(<CloudSyncProvider><Probe /></CloudSyncProvider>);
    expect(mocks.getLocalWorkspaces).toHaveBeenCalledTimes(1);
    await act(() => vi.advanceTimersByTimeAsync(15000));
    expect(mocks.getLocalWorkspaces).toHaveBeenCalledTimes(2);
    expect(mocks.syncNow).not.toHaveBeenCalled();
    expect(mocks.enableCloudSync).not.toHaveBeenCalled();
    unmount();
    await vi.advanceTimersByTimeAsync(30000);
    expect(mocks.getLocalWorkspaces).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each(["unmount", "revoke"])("does not start per-workspace status requests after %s", async (action) => {
    let finish!: (state: { workspaces: { id: string }[] }) => void;
    mocks.getLocalWorkspaces.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    mocks.account = account(true, { kind: "ready" });
    const { rerender, unmount } = render(<CloudSyncProvider><Probe /></CloudSyncProvider>);
    if (action === "unmount") unmount();
    else {
      mocks.account = anonymousAccount();
      rerender(<CloudSyncProvider><Probe /></CloudSyncProvider>);
    }
    await act(async () => { finish({ workspaces: [{ id: "old-workspace" }] }); });
    expect(mocks.getCloudSyncStatus).not.toHaveBeenCalled();
    expect(mocks.getLocalWorkspaces).toHaveBeenCalledTimes(1);
    unmount();
  });
});

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

  it("isolates one workspace status failure without clearing successful workspaces", async () => {
    mocks.account = account(true, { kind: "ready" });
    mocks.getLocalWorkspaces.mockResolvedValue({
      activeWorkspaceId: "workspace-a",
      workspaces: [
        { id: "workspace-a", name: "A" },
        { id: "workspace-b", name: "B" },
        { id: "workspace-c", name: "C" },
      ],
    });
    mocks.getCloudSyncStatus.mockImplementation(async (workspaceId: string) => {
      if (workspaceId === "workspace-b") {
        throw { code: "cloud_sync_storage_failed" };
      }
      return {
        binding: null,
        pendingCount: 0,
        uncertainCount: 0,
        inFlightCount: 0,
        deadCount: 0,
        deadLetters: [],
        conflictCount: 0,
        running: false,
      };
    });

    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("2"));
    expect(screen.getByTestId("workspace-errors")).toHaveTextContent("1");
    expect(screen.getByTestId("error")).toHaveTextContent("none");
  });

  it("refreshes the account before manually retrying a workspace", async () => {
    const events: string[] = [];
    const accountValue = account(true, { kind: "ready" });
    accountValue.refreshAccount = vi.fn(async () => {
      events.push("account");
      return accountSnapshot(accountValue);
    });
    mocks.account = accountValue;
    mocks.syncNow.mockImplementation(async () => { events.push("sync"); });
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "retry" }));

    await waitFor(() => expect(mocks.syncNow).toHaveBeenCalledWith("workspace"));
    expect(events).toEqual(["account", "sync"]);
    expect(accountValue.refreshAccount).toHaveBeenCalledTimes(1);
  });

  it("does not sync when refresh discovers a signed-out account", async () => {
    const accountValue = account(true, { kind: "ready" });
    accountValue.refreshAccount = vi.fn().mockResolvedValue({
      account: { kind: "signedOut" },
      syncContext: { kind: "inactive" },
    });
    mocks.account = accountValue;
    const rendered = render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "retry" }));
    await waitFor(() => expect(accountValue.refreshAccount).toHaveBeenCalledTimes(1));
    mocks.account = anonymousAccount();
    rendered.rerender(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_unauthorized"));
    expect(mocks.syncNow).not.toHaveBeenCalled();
  });

  it("does not sync when refresh discovers that Cloud Sync entitlement is unavailable", async () => {
    const accountValue = account(true, { kind: "ready" });
    accountValue.refreshAccount = vi.fn().mockResolvedValue({
      account: { kind: "signedIn", profile: profile(false) },
      syncContext: { kind: "inactive" },
    });
    mocks.account = accountValue;
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "retry" }));

    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_entitlement_required"));
    expect(mocks.syncNow).not.toHaveBeenCalled();
  });

  it("does not sync when account refresh is temporarily unavailable", async () => {
    const accountValue = account(true, { kind: "ready" });
    accountValue.refreshAccount = vi.fn().mockRejectedValue({ code: "api_unavailable" });
    mocks.account = accountValue;
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "retry" }));

    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_transport_failed"));
    expect(screen.getByTestId("statuses")).toHaveTextContent("1");
    expect(mocks.syncNow).not.toHaveBeenCalled();
  });

  it("does not present an account API rejection as a connectivity failure", async () => {
    const accountValue = account(true, { kind: "ready" });
    accountValue.refreshAccount = vi.fn().mockRejectedValue({ code: "invalid_request" });
    mocks.account = accountValue;
    render(<CloudSyncProvider><Probe /></CloudSyncProvider>);

    await waitFor(() => expect(screen.getByTestId("statuses")).toHaveTextContent("1"));
    fireEvent.click(screen.getByRole("button", { name: "retry" }));

    await waitFor(() => expect(screen.getByTestId("error"))
      .toHaveTextContent("cloud_sync_request_rejected"));
    expect(mocks.syncNow).not.toHaveBeenCalled();
  });
});
