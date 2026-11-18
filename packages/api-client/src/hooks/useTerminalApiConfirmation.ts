import { useEffect } from "react";
import type { ApiRequestTab } from "../model/request-tabs";
import { useApiRequestTabStore } from "../model/api-request-tab-state";
import type { ApiRouteControls } from "./useApiRoutes";

/** Confirmation belongs to one visible request, environment and idle terminal. */
export function useTerminalApiConfirmation(workspaceId: string, tab: ApiRequestTab | null, routing: ApiRouteControls, environmentId: string | null) {
  const tabId = tab?.id;
  useEffect(() => () => {
    if (!tabId) return;
    const store = useApiRequestTabStore.getState();
    if (store.byWorkspace[workspaceId]?.tabs.find((entry) => entry.id === tabId)?.draft.terminalConfirmed) {
      store.updateTabDraft(workspaceId, tabId, { terminalConfirmed: false });
    }
  }, [workspaceId, tabId, environmentId]);
  useEffect(() => {
    if (tabId && tab?.draft.terminalConfirmed && !routing.ready) {
      useApiRequestTabStore.getState().updateTabDraft(workspaceId, tabId, { terminalConfirmed: false });
    }
  }, [workspaceId, tabId, tab?.draft.terminalConfirmed, routing.ready]);
}
