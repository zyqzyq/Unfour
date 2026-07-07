import { create } from "zustand";
import type { DatabaseConnectionSessionState } from "./types";

// Connection lifecycle state for the Database module, partitioned per workspace.
//
// This lives at module scope (not in `DatabasePage` component state) on
// purpose: `DatabasePage` is conditionally unmounted whenever the user
// switches to another module tab (see `DesktopApp.tsx`), and the original
// local `useState` was destroyed on every unmount — so a connection that was
// "connected" silently reverted to "disconnected" after switching away and
// back. Keeping the map here mirrors how `ssh-terminal` keeps its session
// state in `model/terminal-state.ts`, so the connection status survives page
// switches. Partitioning by `workspaceId` keeps each workspace's connections
// isolated: switching workspaces no longer leaks one workspace's live
// connections into another. The backend has no persistent database-session
// registry (each command opens and closes its own pool), so this store is the
// only source of truth for the "connected" indicator — and it is safe to
// keep, because every query re-establishes its own connection using the
// stored credential ref.

const EMPTY_CONNECTION_STATES: Record<string, DatabaseConnectionSessionState> = {};

type DatabaseConnectionStore = {
  byWorkspace: Record<string, Record<string, DatabaseConnectionSessionState>>;
  pruneConnections: (workspaceId: string, liveConnectionIds: Set<string>) => void;
  removeConnection: (workspaceId: string, connectionId: string) => void;
  setConnectionState: (
    workspaceId: string,
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
};

export const useDatabaseConnectionStore = create<DatabaseConnectionStore>(
  (set) => ({
    byWorkspace: {},
    pruneConnections: (workspaceId, liveConnectionIds) =>
      set((state) => {
        const workspace = state.byWorkspace[workspaceId];
        if (!workspace) {
          return state;
        }
        const next: Record<string, DatabaseConnectionSessionState> = {};
        let changed = false;
        for (const [id, value] of Object.entries(workspace)) {
          if (liveConnectionIds.has(id)) {
            next[id] = value;
          } else {
            changed = true;
          }
        }
        return changed
          ? { byWorkspace: { ...state.byWorkspace, [workspaceId]: next } }
          : state;
      }),
    removeConnection: (workspaceId, connectionId) =>
      set((state) => {
        const workspace = state.byWorkspace[workspaceId];
        if (!workspace || !(connectionId in workspace)) {
          return state;
        }
        const next = { ...workspace };
        delete next[connectionId];
        return {
          byWorkspace: { ...state.byWorkspace, [workspaceId]: next },
        };
      }),
    setConnectionState: (workspaceId, connectionId, patch) =>
      set((state) => {
        const workspace = state.byWorkspace[workspaceId] ?? {};
        const current = workspace[connectionId];
        return {
          byWorkspace: {
            ...state.byWorkspace,
            [workspaceId]: {
              ...workspace,
              [connectionId]: {
                message: patch.message ?? current?.message ?? null,
                serverVersion: patch.serverVersion ?? current?.serverVersion ?? null,
                status: patch.status ?? current?.status ?? "disconnected",
                updatedAt: new Date().toISOString(),
              },
            },
          },
        };
      }),
  }),
);

export function resetDatabaseConnectionStore(workspaceId?: string) {
  if (workspaceId === undefined) {
    useDatabaseConnectionStore.setState({ byWorkspace: {} });
    return;
  }
  useDatabaseConnectionStore.setState((state) => {
    const next = { ...state.byWorkspace };
    delete next[workspaceId];
    return { byWorkspace: next };
  });
}

export { EMPTY_CONNECTION_STATES };
