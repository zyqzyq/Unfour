import { useEffect } from "react";
import type { ApiSavedRequest } from "@unfour/command-client";
import { useApiRequestTabStore } from "../model/api-request-tab-state";

export function useApiRequestSelection({
  active, loadedSavedRequests, onActiveSavedRequestChange, restoreSavedRequestId, workspaceId,
}: {
  active: boolean;
  loadedSavedRequests: ApiSavedRequest[] | undefined;
  onActiveSavedRequestChange?: (requestId: string | null) => void;
  // Undefined until the shell has hydrated this workspace's layout.
  restoreSavedRequestId?: string | null;
  workspaceId: string;
}) {
  const selectedId = useApiRequestTabStore((store) => {
    const state = store.byWorkspace[workspaceId];
    if (state) return state.tabs.find((tab) => tab.id === state.activeTabId)?.savedRequestId ?? null;
  });

  useEffect(() => {
    if (restoreSavedRequestId === undefined) return;

    if (selectedId !== undefined) {
      // In-memory tabs take precedence when the layout arrives or the workspace is revisited.
      if (selectedId !== restoreSavedRequestId) onActiveSavedRequestChange?.(selectedId);
      return;
    }

    if (!active || !loadedSavedRequests) return;
    const saved = loadedSavedRequests.find((request) =>
      request.id === restoreSavedRequestId && request.workspaceId === workspaceId && !request.deletedAt,
    );
    // Initialization checks the live store too, in case an explicit open just ran.
    useApiRequestTabStore.getState().initializeWorkspace(workspaceId, saved);
  }, [active, loadedSavedRequests, onActiveSavedRequestChange, restoreSavedRequestId, selectedId, workspaceId]);
}
