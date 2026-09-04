import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  beginAccountSignIn,
  getAccountState,
  isTauriRuntime,
  listenForAccountDeepLinks,
  signOutAccount,
} from "./accountApi";
import { listenForAccountForeground } from "./accountForeground";
import { CLOUD_SYNC_ENTITLEMENT } from "./accountEntitlement";
import { createAccountRefreshController } from "./accountRefresh";
import type {
  AccountMockState,
  AccountProfile,
  AccountState,
  AccountStateSnapshot,
  CloudSyncAccountContextState,
} from "./accountTypes";
import { AccountContext } from "./useAccount";

const MOCK_SIGN_IN_DELAY_MS = 700;
const MOCK_PROFILE: AccountProfile = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  username: "alexchen",
  displayName: "Alex Chen",
  email: "alex@example.com",
  avatarUrl: "https://avatars.githubusercontent.com/u/9919?s=64&v=4",
  entitlements: [
    { code: CLOUD_SYNC_ENTITLEMENT, status: "active", validUntil: null },
  ],
  devices: [{
    id: "550e8400-e29b-41d4-a716-446655440001",
    name: "Unfour Desktop",
    platform: "windows",
    lastSeenAt: null,
    revoked: false,
  }],
};

export function AccountProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AccountState>({ kind: "signedOut" });
  const [syncContext, setSyncContext] = useState<CloudSyncAccountContextState>({ kind: "inactive" });
  const [overlayOpen, setOverlayOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const stateRef = useRef(state);
  const signInTimerRef = useRef<number | null>(null);
  const actionGenerationRef = useRef(0);
  const refreshDisplayGenerationRef = useRef(0);
  const preview = !isTauriRuntime();

  const applyState = useCallback((nextState: AccountState) => {
    stateRef.current = nextState;
    setState(nextState);
  }, []);

  const applySnapshot = useCallback((snapshot: AccountStateSnapshot) => {
    applyState(snapshot.account);
    setSyncContext(snapshot.syncContext);
  }, [applyState]);

  // The factory stores getState; it only invokes it after an async refresh fails.
  // eslint-disable-next-line react-hooks/refs -- this initializer never reads stateRef.current
  const [refreshController] = useState(() =>
    createAccountRefreshController({
      getState: () => stateRef.current,
      loadState: getAccountState,
      setSnapshot: applySnapshot,
    }),
  );

  const clearSignInTimer = useCallback(() => {
    if (signInTimerRef.current !== null) {
      window.clearTimeout(signInTimerRef.current);
      signInTimerRef.current = null;
    }
  }, []);

  const supersedePendingAccountRequests = useCallback(() => {
    actionGenerationRef.current += 1;
    refreshController.invalidate();
  }, [refreshController]);

  const applyExternalSnapshot = useCallback((snapshot: AccountStateSnapshot) => {
    supersedePendingAccountRequests();
    applySnapshot(snapshot);
  }, [applySnapshot, supersedePendingAccountRequests]);

  const applyExternalError = useCallback(() => {
    supersedePendingAccountRequests();
    if (stateRef.current.kind !== "signedIn") applyState({ kind: "error" });
  }, [applyState, supersedePendingAccountRequests]);

  const refreshAccount = useCallback(async (): Promise<AccountStateSnapshot | null> => {
    if (preview) {
      return {
        account: stateRef.current,
        syncContext: stateRef.current.kind === "signedIn"
          ? { kind: "ready" }
          : { kind: "inactive" },
      };
    }
    const displayGeneration = ++refreshDisplayGenerationRef.current;
    setRefreshing(true);
    try {
      return await refreshController.refresh();
    } finally {
      if (displayGeneration === refreshDisplayGenerationRef.current) {
        setRefreshing(false);
      }
    }
  }, [preview, refreshController]);

  useEffect(() => clearSignInTimer, [clearSignInTimer]);
  useEffect(() => {
    if (preview) return;

    let active = true;
    let stopDeepLinks: (() => void) | undefined;
    let stopForeground: (() => void) | undefined;
    const onSnapshot = (snapshot: AccountStateSnapshot) => {
      if (active) applyExternalSnapshot(snapshot);
    };
    const onError = () => {
      if (active) applyExternalError();
    };

    // eslint-disable-next-line react-hooks/set-state-in-effect -- start one external account refresh and show its pending state before awaiting it
    void refreshAccount().catch(() => undefined);
    void listenForAccountDeepLinks(onSnapshot, onError)
      .then((unlisten) => {
        if (active) stopDeepLinks = unlisten;
        else unlisten();
      })
      .catch(onError);
    void listenForAccountForeground(() => {
      if (active) void refreshAccount().catch(() => undefined);
    })
      .then((unlisten) => {
        if (active) stopForeground = unlisten;
        else unlisten();
      })
      // A focus-listener failure must not replace a valid account profile.
      .catch(() => undefined);

    return () => {
      active = false;
      refreshController.invalidate();
      refreshDisplayGenerationRef.current += 1;
      stopDeepLinks?.();
      stopForeground?.();
    };
  }, [applyExternalError, applyExternalSnapshot, preview, refreshAccount, refreshController]);

  const signIn = useCallback(() => {
    clearSignInTimer();
    supersedePendingAccountRequests();
    const actionGeneration = actionGenerationRef.current;
    applyState({ kind: "signingIn" });
    setSyncContext({ kind: "inactive" });
    setOverlayOpen(true);
    if (preview) {
      signInTimerRef.current = window.setTimeout(() => {
        signInTimerRef.current = null;
        if (actionGeneration === actionGenerationRef.current) {
          applyState({ kind: "signedIn", profile: MOCK_PROFILE });
          setSyncContext({ kind: "ready" });
        }
      }, MOCK_SIGN_IN_DELAY_MS);
      return;
    }
    void beginAccountSignIn()
      .then((nextState) => {
        if (actionGeneration === actionGenerationRef.current) applyState(nextState);
      })
      .catch(() => {
        if (actionGeneration === actionGenerationRef.current) applyState({ kind: "error" });
      });
  }, [applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const signOut = useCallback(() => {
    clearSignInTimer();
    supersedePendingAccountRequests();
    const actionGeneration = actionGenerationRef.current;
    setSyncContext({ kind: "inactive" });
    if (preview) {
      applyState({ kind: "signedOut" });
      setOverlayOpen(false);
      return;
    }
    void signOutAccount()
      .then((nextState) => {
        if (actionGeneration !== actionGenerationRef.current) return;
        applyState(nextState);
        setOverlayOpen(false);
      })
      .catch(() => {
        if (actionGeneration === actionGenerationRef.current) applyState({ kind: "error" });
      });
  }, [applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const retry = useCallback(() => {
    if (preview) {
      signIn();
      return;
    }
    clearSignInTimer();
    void refreshAccount().catch(() => undefined);
  }, [clearSignInTimer, preview, refreshAccount, signIn]);

  const setMockState = useCallback((nextState: AccountMockState) => {
    if (!preview) return;
    clearSignInTimer();
    supersedePendingAccountRequests();
    applyState(nextState === "signedIn"
      ? { kind: "signedIn", profile: MOCK_PROFILE }
      : { kind: nextState });
    setSyncContext(nextState === "signedIn" ? { kind: "ready" } : { kind: "inactive" });
  }, [applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const openOverlay = useCallback(() => setOverlayOpen(true), []);
  const value = useMemo(() => ({
    preview,
    state,
    syncContext,
    overlayOpen,
    refreshing,
    setOverlayOpen,
    openOverlay,
    signIn,
    signOut,
    retry,
    refreshAccount,
    setMockState,
  }), [openOverlay, overlayOpen, preview, refreshAccount, refreshing, retry, setMockState, signIn, signOut, state, syncContext]);

  return <AccountContext.Provider value={value}>{children}</AccountContext.Provider>;
}
