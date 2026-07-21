import type { SshTaskSaveInput } from "@unfour/command-client";

export type SshTaskDetailView = "editor" | "history";

export type SshTaskEditorTab = {
  baseline: SshTaskSaveInput | null;
  draft: SshTaskSaveInput | null;
  id: string;
  taskId: string | null;
  view: SshTaskDetailView;
};

export type SshTaskEditorState = {
  activeTabId: string | null;
  tabs: SshTaskEditorTab[];
};

export function createEmptyTaskEditorState(): SshTaskEditorState {
  return { activeTabId: null, tabs: [] };
}

export function openSavedTaskTab(
  state: SshTaskEditorState,
  taskId: string,
): SshTaskEditorState {
  const existing = state.tabs.find((tab) => tab.taskId === taskId);
  if (existing) {
    return { ...state, activeTabId: existing.id };
  }

  const id = savedTaskTabId(taskId);
  return {
    activeTabId: id,
    tabs: [
      ...state.tabs,
      { baseline: null, draft: null, id, taskId, view: "editor" },
    ],
  };
}

export function openNewTaskTab(
  state: SshTaskEditorState,
  id: string,
  draft: SshTaskSaveInput,
): SshTaskEditorState {
  return {
    activeTabId: id,
    tabs: [
      ...state.tabs,
      { baseline: null, draft, id, taskId: null, view: "editor" },
    ],
  };
}

export function hydrateTaskTab(
  state: SshTaskEditorState,
  tabId: string,
  draft: SshTaskSaveInput,
): SshTaskEditorState {
  return updateTab(state, tabId, (tab) => {
    if (tab.draft || tab.taskId !== draft.id) return tab;
    return { ...tab, baseline: draft, draft };
  });
}

export function updateTaskTabDraft(
  state: SshTaskEditorState,
  tabId: string,
  draft: SshTaskSaveInput,
): SshTaskEditorState {
  return updateTab(state, tabId, (tab) => ({ ...tab, draft }));
}

export function updateTaskTabView(
  state: SshTaskEditorState,
  tabId: string,
  view: SshTaskDetailView,
): SshTaskEditorState {
  return updateTab(state, tabId, (tab) => ({ ...tab, view }));
}

export function persistTaskTab(
  state: SshTaskEditorState,
  tabId: string,
  draft: SshTaskSaveInput,
): SshTaskEditorState {
  if (!draft.id) return state;

  const persistedId = savedTaskTabId(draft.id);
  const tabs = state.tabs
    .filter((tab) => tab.id === tabId || tab.id !== persistedId)
    .map((tab) =>
      tab.id === tabId
        ? {
            ...tab,
            baseline: draft,
            draft,
            id: persistedId,
            taskId: draft.id ?? null,
          }
        : tab,
    );

  return {
    activeTabId: state.activeTabId === tabId ? persistedId : state.activeTabId,
    tabs,
  };
}

export function closeTaskTab(
  state: SshTaskEditorState,
  tabId: string,
): SshTaskEditorState {
  const closingIndex = state.tabs.findIndex((tab) => tab.id === tabId);
  if (closingIndex < 0) return state;

  const tabs = state.tabs.filter((tab) => tab.id !== tabId);
  return {
    activeTabId:
      state.activeTabId === tabId
        ? (tabs[Math.min(closingIndex, tabs.length - 1)]?.id ?? null)
        : state.activeTabId,
    tabs,
  };
}

export function removeTaskTabs(
  state: SshTaskEditorState,
  taskId: string,
): SshTaskEditorState {
  const firstRemovedIndex = state.tabs.findIndex((tab) => tab.taskId === taskId);
  if (firstRemovedIndex < 0) return state;

  const removedActiveTab = state.tabs.some(
    (tab) => tab.taskId === taskId && tab.id === state.activeTabId,
  );
  const tabs = state.tabs.filter((tab) => tab.taskId !== taskId);
  return {
    activeTabId: removedActiveTab
      ? (tabs[Math.min(firstRemovedIndex, tabs.length - 1)]?.id ?? null)
      : state.activeTabId,
    tabs,
  };
}

export function isTaskTabDirty(tab: SshTaskEditorTab): boolean {
  if (!tab.draft) return false;
  if (!tab.baseline) return true;
  return JSON.stringify(tab.draft) !== JSON.stringify(tab.baseline);
}

function savedTaskTabId(taskId: string) {
  return `task:${taskId}`;
}

function updateTab(
  state: SshTaskEditorState,
  tabId: string,
  update: (tab: SshTaskEditorTab) => SshTaskEditorTab,
): SshTaskEditorState {
  return {
    ...state,
    tabs: state.tabs.map((tab) => (tab.id === tabId ? update(tab) : tab)),
  };
}
