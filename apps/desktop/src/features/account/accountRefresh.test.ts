import { describe, expect, it, vi } from "vitest";
import { createAccountRefreshController } from "./accountRefresh";
import type { AccountState, AccountStateSnapshot, CloudSyncAccountContextState } from "./accountTypes";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

const signedIn = (id: string): AccountState => ({
  kind: "signedIn",
  profile: {
    id,
    email: `${id}@example.test`,
    username: id,
    displayName: id,
    avatarUrl: null,
    entitlements: [],
    devices: [],
  },
});

const snapshot = (
  account: AccountState,
  syncContext: CloudSyncAccountContextState = { kind: "ready" },
): AccountStateSnapshot => ({ account, syncContext });

describe("createAccountRefreshController", () => {
  it("loads account_get_state and applies the returned state", async () => {
    let state: AccountState = { kind: "signedOut" };
    let syncContext: CloudSyncAccountContextState = { kind: "inactive" };
    const nextState = signedIn("fresh");
    const nextSnapshot = snapshot(nextState);
    const loadState = vi.fn().mockResolvedValue(nextSnapshot);
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState,
      setSnapshot: (next) => {
        state = next.account;
        syncContext = next.syncContext;
      },
    });

    await expect(controller.refresh()).resolves.toEqual(nextSnapshot);
    expect(loadState).toHaveBeenCalledTimes(1);
    expect(state).toEqual(nextState);
    expect(syncContext).toEqual({ kind: "ready" });
  });

  it("coalesces concurrent refreshes into one request", async () => {
    let state: AccountState = { kind: "signedOut" };
    const pending = deferred<AccountStateSnapshot>();
    const loadState = vi.fn(() => pending.promise);
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState,
      setSnapshot: (next) => { state = next.account; },
    });

    const first = controller.refresh();
    const second = controller.refresh();
    expect(second).toBe(first);
    await Promise.resolve();
    expect(loadState).toHaveBeenCalledTimes(1);
    pending.resolve(snapshot(signedIn("paid")));
    await Promise.all([first, second]);
    expect(state).toEqual(signedIn("paid"));
  });

  it("does not let an invalidated response overwrite a newer request", async () => {
    let state: AccountState = { kind: "signedOut" };
    const older = deferred<AccountStateSnapshot>();
    const newer = deferred<AccountStateSnapshot>();
    const loadState = vi.fn()
      .mockImplementationOnce(() => older.promise)
      .mockImplementationOnce(() => newer.promise);
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState,
      setSnapshot: (next) => { state = next.account; },
    });

    const olderRequest = controller.refresh();
    await Promise.resolve();
    controller.invalidate();
    const newerRequest = controller.refresh();
    await Promise.resolve();
    newer.resolve(snapshot(signedIn("newer")));
    await newerRequest;
    older.resolve(snapshot(signedIn("older")));
    await expect(olderRequest).resolves.toBeNull();
    expect(state).toEqual(signedIn("newer"));
  });

  it("keeps a valid signed-in profile when refresh fails", async () => {
    let state = signedIn("existing");
    const pending = deferred<AccountStateSnapshot>();
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState: () => pending.promise,
      setSnapshot: (next) => { state = next.account; },
    });

    const request = controller.refresh();
    pending.reject(new Error("offline"));
    await expect(request).rejects.toThrow("offline");
    expect(state).toEqual(signedIn("existing"));
  });

  it("surfaces an initial refresh failure when no signed-in profile exists", async () => {
    let state: AccountState = { kind: "signedOut" };
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState: () => Promise.reject(new Error("offline")),
      setSnapshot: (next) => { state = next.account; },
    });

    await expect(controller.refresh()).rejects.toThrow("offline");
    expect(state).toEqual({ kind: "error" });
  });
});
