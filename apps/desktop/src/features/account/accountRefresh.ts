import type { AccountState, AccountStateSnapshot } from "./accountTypes";

export interface AccountRefreshController {
  invalidate(): void;
  refresh(): Promise<AccountStateSnapshot | null>;
}

/**
 * Coalesces refreshes within one account generation. Auth and sign-out flows
 * invalidate the generation so an older /v1/me response cannot overwrite a
 * newer authoritative state.
 */
export function createAccountRefreshController({
  getState,
  loadState,
  setSnapshot,
}: {
  getState: () => AccountState;
  loadState: () => Promise<AccountStateSnapshot>;
  setSnapshot: (snapshot: AccountStateSnapshot) => void;
}): AccountRefreshController {
  let generation = 0;
  let inFlight: { generation: number; request: Promise<AccountStateSnapshot | null> } | null = null;

  return {
    invalidate() {
      generation += 1;
    },
    refresh() {
      if (inFlight?.generation === generation) return inFlight.request;

      const requestGeneration = generation;
      const request = Promise.resolve()
        .then(loadState)
        .then((snapshot) => {
          if (requestGeneration !== generation) return null;
          setSnapshot(snapshot);
          return snapshot;
        })
        .catch((error: unknown) => {
          if (requestGeneration === generation && getState().kind !== "signedIn") {
            setSnapshot({ account: { kind: "error" }, syncContext: { kind: "inactive" } });
          }
          throw error;
        })
        .finally(() => {
          if (inFlight?.request === request) inFlight = null;
        });

      inFlight = { generation: requestGeneration, request };
      return request;
    },
  };
}
