// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { AccountProfile, AccountState, AccountStateSnapshot, CloudSyncAccountContextState } from "./accountTypes";

const mocks = vi.hoisted(() => ({
  beginAccountSignIn: vi.fn(),
  deepLinkError: undefined as (() => void) | undefined,
  deepLinkState: undefined as ((snapshot: AccountStateSnapshot) => void) | undefined,
  focus: undefined as (() => void) | undefined,
  getAccountState: vi.fn(),
  stopDeepLinks: vi.fn(),
  stopForeground: vi.fn(),
  signOutAccount: vi.fn(),
}));

vi.mock("./accountApi", () => ({
  beginAccountSignIn: mocks.beginAccountSignIn,
  getAccountState: mocks.getAccountState,
  isTauriRuntime: () => true,
  listenForAccountDeepLinks: vi.fn(async (
    onState: (snapshot: AccountStateSnapshot) => void,
    onError: () => void,
  ) => {
    mocks.deepLinkState = onState;
    mocks.deepLinkError = onError;
    return mocks.stopDeepLinks;
  }),
  signOutAccount: mocks.signOutAccount,
}));
vi.mock("./accountForeground", () => ({
  listenForAccountForeground: vi.fn(async (onForeground: () => void) => {
    mocks.focus = onForeground;
    return mocks.stopForeground;
  }),
}));

import { CLOUD_SYNC_ENTITLEMENT, hasActiveEntitlement } from "./accountEntitlement";
import { AccountProvider } from "./AccountProvider";
import { useAccount } from "./useAccount";

function profile(id: string, hasCloudSyncCapability: boolean): AccountProfile {
  return {
    id,
    email: `${id}@example.test`,
    username: id,
    displayName: id,
    avatarUrl: null,
    entitlements: hasCloudSyncCapability
      ? [{ code: "cloud_sync", status: "active", validUntil: null }]
      : [],
    devices: [],
  };
}

function snapshot(
  account: AccountState,
  syncContext: CloudSyncAccountContextState = { kind: "ready" },
): AccountStateSnapshot {
  return { account, syncContext };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function Probe({ children }: { children?: ReactNode }) {
  const account = useAccount();
  const id = account.state.kind === "signedIn" ? account.state.profile.id : account.state.kind;
  const plan = account.state.kind === "signedIn"
    && hasActiveEntitlement(account.state.profile.entitlements, CLOUD_SYNC_ENTITLEMENT)
    ? "pro"
    : "free";
  const sync = account.syncContext.kind === "error"
    ? `${account.syncContext.kind}:${account.syncContext.code}`
    : account.syncContext.kind;
  return (
    <div>
      <span data-testid="account-id">{id}</span>
      <span data-testid="plan">{plan}</span>
      <span data-testid="sync-context">{sync}</span>
      <button onClick={() => void account.refreshAccount().catch(() => undefined)} type="button">
        refresh
      </button>
      <button onClick={account.signOut} type="button">sign out</button>
      {children}
    </div>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.deepLinkError = undefined;
  mocks.deepLinkState = undefined;
  mocks.focus = undefined;
  mocks.beginAccountSignIn.mockResolvedValue({ kind: "signingIn" });
  mocks.signOutAccount.mockResolvedValue({ kind: "signedOut" });
});
afterEach(cleanup);

describe("AccountProvider refresh", () => {
  it("refreshes on focus and coalesces simultaneous foreground signals", async () => {
    const pending = deferred<AccountStateSnapshot>();
    mocks.getAccountState
      .mockResolvedValueOnce(snapshot({ kind: "signedIn", profile: profile("free", false) }, { kind: "inactive" }))
      .mockImplementationOnce(() => pending.promise);
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("free"));

    mocks.focus?.();
    mocks.focus?.();
    await waitFor(() => expect(mocks.getAccountState).toHaveBeenCalledTimes(2));
    pending.resolve(snapshot({ kind: "signedIn", profile: profile("paid", true) }));
    await waitFor(() => expect(screen.getByTestId("plan")).toHaveTextContent("pro"));
    expect(screen.getByTestId("sync-context")).toHaveTextContent("ready");
  });

  it("keeps signed-in state when a manual refresh fails", async () => {
    mocks.getAccountState
      .mockResolvedValueOnce(snapshot({ kind: "signedIn", profile: profile("existing", true) }))
      .mockRejectedValueOnce(new Error("offline"));
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("existing"));

    fireEvent.click(screen.getByRole("button", { name: "refresh" }));
    await waitFor(() => expect(mocks.getAccountState).toHaveBeenCalledTimes(2));
    expect(screen.getByTestId("account-id")).toHaveTextContent("existing");
  });

  it("does not let an older refresh overwrite an auth deep-link state", async () => {
    const older = deferred<AccountStateSnapshot>();
    mocks.getAccountState
      .mockResolvedValueOnce(snapshot({ kind: "signedIn", profile: profile("free", false) }, { kind: "inactive" }))
      .mockImplementationOnce(() => older.promise);
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("free"));

    mocks.focus?.();
    await waitFor(() => expect(mocks.getAccountState).toHaveBeenCalledTimes(2));
    mocks.deepLinkState?.(snapshot({ kind: "signedIn", profile: profile("auth-newer", true) }));
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("auth-newer"));
    older.resolve(snapshot({ kind: "signedIn", profile: profile("stale", false) }, { kind: "inactive" }));
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("auth-newer"));
  });

  it("cleans up deep-link and focus listeners on unmount", async () => {
    mocks.getAccountState.mockResolvedValue(snapshot({ kind: "signedOut" }, { kind: "inactive" }));
    const view = render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(mocks.focus).toBeTypeOf("function"));
    view.unmount();
    expect(mocks.stopDeepLinks).toHaveBeenCalledTimes(1);
    expect(mocks.stopForeground).toHaveBeenCalledTimes(1);
  });

  it("keeps the active Cloud Sync capability when local context activation fails", async () => {
    mocks.getAccountState.mockResolvedValue(snapshot(
      { kind: "signedIn", profile: profile("paid", true) },
      { kind: "error", code: "cloud_sync_storage_failed" },
    ));
    render(<AccountProvider><Probe /></AccountProvider>);

    await waitFor(() => expect(screen.getByTestId("plan")).toHaveTextContent("pro"));
    expect(screen.getByTestId("sync-context")).toHaveTextContent("error:cloud_sync_storage_failed");
  });

  it("shows the refreshed Free plan when sync cleanup fails after revocation", async () => {
    mocks.getAccountState
      .mockResolvedValueOnce(snapshot({ kind: "signedIn", profile: profile("paid", true) }))
      .mockResolvedValueOnce(snapshot(
        { kind: "signedIn", profile: profile("free", false) },
        { kind: "error", code: "cloud_sync_storage_failed" },
      ));
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("plan")).toHaveTextContent("pro"));

    fireEvent.click(screen.getByRole("button", { name: "refresh" }));
    await waitFor(() => expect(screen.getByTestId("plan")).toHaveTextContent("free"));
    expect(screen.getByTestId("sync-context")).toHaveTextContent("error:cloud_sync_storage_failed");
  });

  it("keeps the normal sign-out state and closes the sync context", async () => {
    mocks.getAccountState.mockResolvedValue(snapshot({ kind: "signedIn", profile: profile("paid", true) }));
    render(<AccountProvider><Probe /></AccountProvider>);
    await waitFor(() => expect(screen.getByTestId("plan")).toHaveTextContent("pro"));

    fireEvent.click(screen.getByRole("button", { name: "sign out" }));
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("signedOut"));
    expect(screen.getByTestId("sync-context")).toHaveTextContent("inactive");
  });

  it("keeps descendants mounted when the account API is unavailable", async () => {
    mocks.getAccountState.mockRejectedValue(new Error("command unavailable"));

    render(<AccountProvider><Probe><span>local desktop content</span></Probe></AccountProvider>);

    expect(screen.getByText("local desktop content")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("account-id")).toHaveTextContent("error"));
    expect(screen.getByText("local desktop content")).toBeInTheDocument();
  });
});
