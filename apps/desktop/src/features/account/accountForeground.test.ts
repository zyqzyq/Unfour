import { describe, expect, it, vi } from "vitest";
import { listenForAccountForeground, type AccountFocusSubscriber } from "./accountForeground";
import { createAccountRefreshController } from "./accountRefresh";
import type { AccountState, AccountStateSnapshot } from "./accountTypes";

describe("listenForAccountForeground", () => {
  it("refreshes on focus, ignores blur, deduplicates rapid focus, and cleans up", async () => {
    let focusListener: ((focused: boolean) => void) | undefined;
    const unlisten = vi.fn();
    const subscribe: AccountFocusSubscriber = vi.fn(async (listener) => {
      focusListener = listener;
      return unlisten;
    });
    let state: AccountState = { kind: "signedOut" };
    let resolveRefresh!: (state: AccountStateSnapshot) => void;
    const loadState = vi.fn(() => new Promise<AccountStateSnapshot>((resolve) => {
      resolveRefresh = resolve;
    }));
    const controller = createAccountRefreshController({
      getState: () => state,
      loadState,
      setSnapshot: (next) => { state = next.account; },
    });
    const stop = await listenForAccountForeground(
      () => { void controller.refresh().catch(() => undefined); },
      subscribe,
    );

    focusListener?.(false);
    await Promise.resolve();
    expect(loadState).not.toHaveBeenCalled();
    focusListener?.(true);
    focusListener?.(true);
    await Promise.resolve();
    expect(loadState).toHaveBeenCalledTimes(1);

    resolveRefresh({ account: { kind: "signedOut" }, syncContext: { kind: "inactive" } });
    await Promise.resolve();
    stop();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
