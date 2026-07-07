import { useCallback, useMemo } from "react";
import type { DatabaseTable } from "@unfour/command-client";
import { DEFAULT_WORKSPACE_TAB_STATE, useDatabaseTabStore } from "../model/database-tab-state";
import type {
  DatabaseQueryWorkspaceTab,
  DatabaseTableWorkspaceTab,
} from "../model/types";

export {
  databaseTableTabId,
} from "../model/database-tab-state";

type QueryTabInput = {
  catalog?: string | null;
  connectionId?: string | null;
  schema?: string | null;
  sql?: string;
};

type DatabaseTabsOptions = {
  formatQueryTitle?: (index: number) => string;
  workspaceId?: string;
};

type QueryTabPatch =
  | Partial<Omit<DatabaseQueryWorkspaceTab, "id" | "kind">>
  | ((tab: DatabaseQueryWorkspaceTab) => Partial<Omit<DatabaseQueryWorkspaceTab, "id" | "kind">>);

type TableTabPatch =
  | Partial<Omit<DatabaseTableWorkspaceTab, "id" | "kind">>
  | ((tab: DatabaseTableWorkspaceTab) => Partial<Omit<DatabaseTableWorkspaceTab, "id" | "kind">>);

function defaultQueryTitle(index: number) {
  return `Query ${index}`;
}

export function useDatabaseTabs(options: DatabaseTabsOptions = {}) {
  const workspaceId = options.workspaceId ?? "default";
  const formatQueryTitle = options.formatQueryTitle ?? defaultQueryTitle;
  const slice = useDatabaseTabStore(
    (state) => state.byWorkspace[workspaceId] ?? DEFAULT_WORKSPACE_TAB_STATE,
  );
  const activeTabId = slice.activeTabId;
  const tabs = slice.tabs;
  const activeTab = useMemo(
    () => tabs.find((tab) => tab.id === activeTabId) ?? tabs[0] ?? null,
    [activeTabId, tabs],
  );

  const openQueryTab = useCallback(
    (input: QueryTabInput = {}) =>
      useDatabaseTabStore.getState().openQueryTab(workspaceId, input, formatQueryTitle),
    [workspaceId, formatQueryTitle],
  );
  const openTableTab = useCallback(
    (connectionId: string, table: DatabaseTable, segment: "data" | "structure" = "data") =>
      useDatabaseTabStore.getState().openTableTab(workspaceId, connectionId, table, segment),
    [workspaceId],
  );
  const closeTab = useCallback(
    (tabId: string) => useDatabaseTabStore.getState().closeTab(workspaceId, tabId, formatQueryTitle),
    [workspaceId, formatQueryTitle],
  );
  const setActiveTabId = useCallback(
    (tabId: string) => useDatabaseTabStore.getState().setActiveTabId(workspaceId, tabId),
    [workspaceId],
  );
  const reorderTabs = useCallback(
    (fromIndex: number, toIndex: number) =>
      useDatabaseTabStore.getState().reorderTabs(workspaceId, fromIndex, toIndex),
    [workspaceId],
  );
  const updateQueryTab = useCallback(
    (tabId: string, patch: QueryTabPatch) =>
      useDatabaseTabStore.getState().updateQueryTab(workspaceId, tabId, patch),
    [workspaceId],
  );
  const updateTableTab = useCallback(
    (tabId: string, patch: TableTabPatch) =>
      useDatabaseTabStore.getState().updateTableTab(workspaceId, tabId, patch),
    [workspaceId],
  );
  const removeConnectionTabs = useCallback(
    (connectionId: string) =>
      useDatabaseTabStore.getState().removeConnectionTabs(workspaceId, connectionId, formatQueryTitle),
    [workspaceId, formatQueryTitle],
  );

  return {
    activeTab,
    activeTabId,
    closeTab,
    openQueryTab,
    openTableTab,
    removeConnectionTabs,
    reorderTabs,
    setActiveTabId,
    tabs,
    updateQueryTab,
    updateTableTab,
  };
}
