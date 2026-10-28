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
import { accountFailure, type AccountFailure } from "./accountFailure";
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
  const [failure, setFailure] = useState<AccountFailure | null>(null);
  const failureRef = useRef(failure);
  const listenerFailureRef = useRef<AccountFailure | null>(null);
  const stateRef = useRef(state);
  const signInTimerRef = useRef<number | null>(null);
  const actionGenerationRef = useRef(0);
  const refreshDisplayGenerationRef = useRef(0);
  const preview = !isTauriRuntime();

  const applyState = useCallback((nextState: AccountState) => {
    stateRef.current = nextState;
    setState(nextState);
  }, []);

  const applyFailure = useCallback((nextFailure: AccountFailure | null) => {
    const visibleFailure = nextFailure ?? listenerFailureRef.current;
    failureRef.current = visibleFailure;
    setFailure(visibleFailure);
  }, []);

  const applySnapshot = useCallback((snapshot: AccountStateSnapshot) => {
    applyState(snapshot.account);
    setSyncContext(snapshot.syncContext);
    applyFailure(snapshot.failure ? accountFailure(snapshot.failure, "accountRefresh") : null);
  }, [applyFailure, applyState]);

  // The factory stores getState; it only invokes it after an async refresh fails.
  // eslint-disable-next-line react-hooks/refs -- this initializer never reads stateRef.current
  const [refreshController] = useState(() =>
    createAccountRefreshController({
      getState: () => stateRef.current,
      loadState: getAccountState,
      setSnapshot: applySnapshot,
      onError: (error) => applyFailure(accountFailure(error, "accountRefresh")),
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
    // A successfully delivered callback proves that the live listener works.
    listenerFailureRef.current = null;
    supersedePendingAccountRequests();
    applySnapshot(snapshot);
  }, [applySnapshot, supersedePendingAccountRequests]);

  const applyExternalError = useCallback((error: unknown) => {
    const nextFailure = accountFailure(error, "callbackValidation");
    // A replay or a superseded exchange cannot invalidate a newer login action.
    // A cold-start callback with no pending PKCE still surfaces while signed out.
    if (nextFailure.code === "no_pending_authorization"
      && (stateRef.current.kind === "signedIn" || stateRef.current.kind === "signingIn"
        || failureRef.current !== null)) return;
    const preservesPending = stateRef.current.kind === "signingIn"
      && ["state_mismatch", "invalid_deep_link"].includes(nextFailure.code);
    if (preservesPending) refreshController.invalidate();
    else supersedePendingAccountRequests();
    if (nextFailure.stage === "callbackListener") listenerFailureRef.current = nextFailure;
    applyFailure(nextFailure);
    if (stateRef.current.kind !== "signedIn" && !preservesPending) applyState({ kind: "error" });
  }, [applyFailure, applyState, refreshController, supersedePendingAccountRequests]);

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
    const onError = (error: unknown) => {
      if (active) applyExternalError(error);
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
      // Foreground refresh must not hide a failed login or race an exchange.
      if (active && stateRef.current.kind !== "signingIn"
        && (!failureRef.current || failureRef.current.recovery === "refreshAccount")) {
        void refreshAccount().catch(() => undefined);
      }
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
    if (listenerFailureRef.current || failureRef.current?.recovery === "restartApp") {
      setOverlayOpen(true);
      return;
    }
    clearSignInTimer();
    supersedePendingAccountRequests();
    const actionGeneration = actionGenerationRef.current;
    applyState({ kind: "signingIn" });
    applyFailure(null);
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
      .catch((error: unknown) => {
        if (actionGeneration === actionGenerationRef.current) {
          applyState({ kind: "error" });
          applyFailure(accountFailure(error, "prepareSignIn"));
        }
      });
  }, [applyFailure, applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const signOut = useCallback(() => {
    clearSignInTimer();
    supersedePendingAccountRequests();
    const actionGeneration = actionGenerationRef.current;
    applyFailure(null);
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
      .catch((error: unknown) => {
        if (actionGeneration === actionGenerationRef.current) {
          applyState({ kind: "error" });
          applyFailure(accountFailure(error, "signOut"));
        }
      });
  }, [applyFailure, applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const retry = useCallback(() => {
    const recovery = failureRef.current?.recovery;
    if (recovery === "restartApp") return;
    if (recovery === "retrySignOut") {
      signOut();
      return;
    }
    if (preview || recovery === "restartSignIn" || stateRef.current.kind === "signingIn") {
      signIn();
      return;
    }
    clearSignInTimer();
    void refreshAccount().catch(() => undefined);
  }, [clearSignInTimer, preview, refreshAccount, signIn, signOut]);

  const setMockState = useCallback((nextState: AccountMockState) => {
    if (!preview) return;
    clearSignInTimer();
    supersedePendingAccountRequests();
    applyFailure(nextState === "error" ? accountFailure(null, "codeExchange") : null);
    applyState(nextState === "signedIn"
      ? { kind: "signedIn", profile: MOCK_PROFILE }
      : { kind: nextState });
    setSyncContext(nextState === "signedIn" ? { kind: "ready" } : { kind: "inactive" });
  }, [applyFailure, applyState, clearSignInTimer, preview, supersedePendingAccountRequests]);

  const openOverlay = useCallback(() => setOverlayOpen(true), []);
  const value = useMemo(() => ({
    preview,
    state,
    failure,
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
  }), [failure, openOverlay, overlayOpen, preview, refreshAccount, refreshing, retry, setMockState, signIn, signOut, state, syncContext]);

  return <AccountContext.Provider value={value}>{children}</AccountContext.Provider>;
}
