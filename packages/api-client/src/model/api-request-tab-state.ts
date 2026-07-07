import { create } from "zustand";
import type {
  ApiHistoryDetail,
  ApiRequestInput,
  ApiResponse,
  ApiSavedRequest,
} from "@unfour/command-client";
import {
  closeApiTab,
  closeApiTabs,
  completeTabSave,
  completeTabSend,
  createNewRequestTab,
  emptyApiTabsState,
  failTabSave,
  failTabSend,
  openHistoryRequest,
  openSavedRequest,
  setActiveApiTab,
  setApiSplitDirection,
  setTabRequestPanel,
  setTabResponsePanel,
  startTabSave,
  startTabSend,
  updateTabDraft,
  type ApiTabsState,
} from "./request-tabs";
import type {
  ApiSplitDirection,
  RequestDraft,
  RequestParamsTab,
  ResponseTab,
} from "./types";

// Tab state for the API Client module, partitioned per workspace so that
// switching workspaces keeps each workspace's open request tabs and drafts
// isolated and preserved (mirrors how `useDatabaseTabStore` isolates DB tabs).
// The store lives at module scope so the tabs survive `ApiClientPage` being
// remounted or hidden when the user switches modules or workspaces.
type ApiTabWorkspaceState = ApiTabsState & {
  // Per-workspace counter for generating `new:N` tab ids.
  nextNewIndex: number;
};

type ApiTabStore = {
  byWorkspace: Record<string, ApiTabWorkspaceState>;
  closeTab: (workspaceId: string, tabId: string) => void;
  closeTabs: (workspaceId: string, tabIds: string[]) => void;
  completeTabSave: (workspaceId: string, tabId: string, saved: ApiSavedRequest) => void;
  completeTabSend: (workspaceId: string, tabId: string, response: ApiResponse) => void;
  failTabSave: (workspaceId: string, tabId: string, error: string) => void;
  failTabSend: (workspaceId: string, tabId: string, error: string) => void;
  newRequest: (workspaceId: string) => void;
  openHistory: (workspaceId: string, history: ApiHistoryDetail) => void;
  openSaved: (workspaceId: string, saved: ApiSavedRequest) => void;
  setActiveTab: (workspaceId: string, tabId: string) => void;
  setApiSplitDirection: (workspaceId: string, splitDirection: ApiSplitDirection) => void;
  setRequestPanel: (workspaceId: string, tabId: string, requestTab: RequestParamsTab) => void;
  setResponsePanel: (workspaceId: string, tabId: string, responseTab: ResponseTab) => void;
  startTabSave: (workspaceId: string, tabId: string) => void;
  startTabSend: (workspaceId: string, tabId: string, request: ApiRequestInput | null) => void;
  updateTabDraft: (workspaceId: string, tabId: string, patch: Partial<RequestDraft>) => void;
};

function createDefaultApiTabState(workspaceId: string): ApiTabWorkspaceState {
  return {
    ...createNewRequestTab(emptyApiTabsState(workspaceId), "new:1"),
    nextNewIndex: 2,
  };
}

// Stable fallback used by selectors when a workspace has no materialized
// slice yet. A constant (not a freshly-built object) is required so the
// zustand selector does not return a new reference on every call, which would
// trip React's "getSnapshot should be cached" check. The embedded
// `workspaceId` is irrelevant because reducers always operate on the real
// `byWorkspace[workspaceId]` slice, never on this placeholder.
const DEFAULT_API_TAB_STATE: ApiTabWorkspaceState = createDefaultApiTabState("");

function withWorkspace(
  state: ApiTabStore,
  workspaceId: string,
  updater: (slice: ApiTabWorkspaceState) => ApiTabWorkspaceState,
): Pick<ApiTabStore, "byWorkspace"> {
  const slice = state.byWorkspace[workspaceId] ?? createDefaultApiTabState(workspaceId);
  return { byWorkspace: { ...state.byWorkspace, [workspaceId]: updater(slice) } };
}

export const useApiRequestTabStore = create<ApiTabStore>((set) => ({
  byWorkspace: {},
  closeTab: (workspaceId, tabId) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...closeApiTab(slice, tabId),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  closeTabs: (workspaceId, tabIds) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...closeApiTabs(slice, tabIds),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  completeTabSave: (workspaceId, tabId, saved) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...completeTabSave(slice, tabId, saved),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  completeTabSend: (workspaceId, tabId, response) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...completeTabSend(slice, tabId, response),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  failTabSave: (workspaceId, tabId, error) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...failTabSave(slice, tabId, error),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  failTabSend: (workspaceId, tabId, error) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...failTabSend(slice, tabId, error),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  newRequest: (workspaceId) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => {
        const id = `new:${slice.nextNewIndex}`;
        return {
          ...createNewRequestTab(slice, id),
          nextNewIndex: slice.nextNewIndex + 1,
        };
      }),
    ),
  openHistory: (workspaceId, history) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...openHistoryRequest(slice, history),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  openSaved: (workspaceId, saved) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...openSavedRequest(slice, saved),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  setActiveTab: (workspaceId, tabId) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...setActiveApiTab(slice, tabId),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  setApiSplitDirection: (workspaceId, splitDirection) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...setApiSplitDirection(slice, splitDirection),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  setRequestPanel: (workspaceId, tabId, requestTab) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...setTabRequestPanel(slice, tabId, requestTab),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  setResponsePanel: (workspaceId, tabId, responseTab) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...setTabResponsePanel(slice, tabId, responseTab),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  startTabSave: (workspaceId, tabId) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...startTabSave(slice, tabId),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  startTabSend: (workspaceId, tabId, request) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...startTabSend(slice, tabId, request),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
  updateTabDraft: (workspaceId, tabId, patch) =>
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...updateTabDraft(slice, tabId, patch),
        nextNewIndex: slice.nextNewIndex,
      })),
    ),
}));

export function resetApiRequestTabStore(workspaceId?: string) {
  if (workspaceId === undefined) {
    useApiRequestTabStore.setState({ byWorkspace: {} });
    return;
  }
  useApiRequestTabStore.setState((state) => {
    const next = { ...state.byWorkspace };
    delete next[workspaceId];
    return { byWorkspace: next };
  });
}

export { DEFAULT_API_TAB_STATE };
