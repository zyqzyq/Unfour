import { create } from "zustand";
import type { DatabaseTable } from "@unfour/command-client";
import { databaseTableTreeId } from "./database-tree";
import { defaultSql } from "./database-state";
import {
  emptyTableQuery,
  type DatabaseQueryWorkspaceTab,
  type DatabaseTableWorkspaceTab,
  type DatabaseWorkspaceTab,
  type DatabaseWorkspaceTabId,
  type TableSegment,
} from "./types";

type QueryTabInput = {
  catalog?: string | null;
  connectionId?: string | null;
  schema?: string | null;
  sql?: string;
};

type QueryTabPatch =
  | Partial<Omit<DatabaseQueryWorkspaceTab, "id" | "kind">>
  | ((tab: DatabaseQueryWorkspaceTab) => Partial<Omit<DatabaseQueryWorkspaceTab, "id" | "kind">>);

type TableTabPatch =
  | Partial<Omit<DatabaseTableWorkspaceTab, "id" | "kind">>
  | ((tab: DatabaseTableWorkspaceTab) => Partial<Omit<DatabaseTableWorkspaceTab, "id" | "kind">>);

type DatabaseTabWorkspaceState = {
  activeTabId: DatabaseWorkspaceTabId;
  nextQueryIndex: number;
  tabs: DatabaseWorkspaceTab[];
};

// Tab state for the Database module, partitioned per workspace so that
// switching workspaces keeps each workspace's open tabs/queries isolated
// (the same way `useDatabaseConnectionStore` isolates connections). The store
// itself lives at module scope so the tabs survive `DatabasePage` being
// unmounted when the user switches to another module tab.
type DatabaseTabStore = {
  byWorkspace: Record<string, DatabaseTabWorkspaceState>;
  closeTab: (
    workspaceId: string,
    tabId: DatabaseWorkspaceTabId,
    formatQueryTitle?: (index: number) => string,
  ) => void;
  openQueryTab: (
    workspaceId: string,
    input?: QueryTabInput,
    formatQueryTitle?: (index: number) => string,
  ) => DatabaseWorkspaceTabId;
  openTableTab: (
    workspaceId: string,
    connectionId: string,
    table: DatabaseTable,
    segment?: TableSegment,
  ) => DatabaseWorkspaceTabId;
  removeConnectionTabs: (
    workspaceId: string,
    connectionId: string,
    formatQueryTitle?: (index: number) => string,
  ) => void;
  reorderTabs: (workspaceId: string, fromIndex: number, toIndex: number) => void;
  setActiveTabId: (workspaceId: string, tabId: DatabaseWorkspaceTabId) => void;
  updateQueryTab: (workspaceId: string, tabId: DatabaseWorkspaceTabId, patch: QueryTabPatch) => void;
  updateTableTab: (workspaceId: string, tabId: DatabaseWorkspaceTabId, patch: TableTabPatch) => void;
};

export function databaseTableTabId(connectionId: string, table: DatabaseTable) {
  return `database-tab:${databaseTableTreeId(connectionId, table)}`;
}

function defaultQueryTitle(index: number) {
  return `Query ${index}`;
}

function createQueryTab(
  index: number,
  input: QueryTabInput = {},
  formatQueryTitle: (index: number) => string = defaultQueryTitle,
): DatabaseQueryWorkspaceTab {
  return {
    catalog: input.catalog ?? null,
    connectionId: input.connectionId ?? null,
    error: null,
    id: `database-query-${index}`,
    kind: "query",
    pendingConfirmation: false,
    result: null,
    resultTab: "results",
    schema: input.schema ?? null,
    sql: input.sql ?? defaultSql,
    title: formatQueryTitle(index),
  };
}

function createTableTab(
  id: string,
  connectionId: string,
  table: DatabaseTable,
  segment: TableSegment,
): DatabaseTableWorkspaceTab {
  return {
    connectionId,
    error: null,
    id,
    kind: "table",
    queryResult: null,
    segment,
    structureTab: "ddl",
    table,
    tableQuery: { ...emptyTableQuery },
    tableView: null,
    title: table.name,
  };
}

function allocateNextQueryIndex(current: number): { index: number; next: number } {
  return { index: current, next: current + 1 };
}

function createDefaultWorkspaceState(): DatabaseTabWorkspaceState {
  const tab = createQueryTab(1, {}, defaultQueryTitle);
  return { activeTabId: tab.id, nextQueryIndex: 2, tabs: [tab] };
}

// Stable fallback used by selectors when a workspace has no materialized
// slice yet. A constant (not a freshly-built object) is required so the
// zustand selector does not return a new reference on every call, which would
// trip React's "getSnapshot should be cached" check.
const DEFAULT_WORKSPACE_TAB_STATE: DatabaseTabWorkspaceState = createDefaultWorkspaceState();

function withWorkspace(
  state: DatabaseTabStore,
  workspaceId: string,
  updater: (slice: DatabaseTabWorkspaceState) => DatabaseTabWorkspaceState,
): Pick<DatabaseTabStore, "byWorkspace"> {
  const slice = state.byWorkspace[workspaceId] ?? createDefaultWorkspaceState();
  return { byWorkspace: { ...state.byWorkspace, [workspaceId]: updater(slice) } };
}

export const useDatabaseTabStore = create<DatabaseTabStore>((set, get) => ({
  byWorkspace: {},
  closeTab: (workspaceId, tabId, formatQueryTitle) => {
    const fmt = formatQueryTitle ?? defaultQueryTitle;
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => {
        // Allocate the replacement index outside the store updater so the
        // counter advances exactly once per user action (mirrors the previous
        // hook, where React.StrictMode double-invoked updaters and used to
        // skip a query number).
        const needsFallback = slice.tabs.filter((tab) => tab.id !== tabId).length === 0;
        const { index: fallbackIndex, next: fallbackNext } = needsFallback
          ? allocateNextQueryIndex(slice.nextQueryIndex)
          : { index: slice.nextQueryIndex, next: slice.nextQueryIndex };
        const fallbackTab = needsFallback ? createQueryTab(fallbackIndex, {}, fmt) : null;
        const tabs = slice.tabs.filter((tab) => tab.id !== tabId);
        const nextTabs = tabs.length
          ? tabs
          : fallbackTab
            ? [fallbackTab]
            : [createQueryTab(slice.nextQueryIndex, {}, fmt)];
        const activeTabId =
          slice.activeTabId === tabId
            ? (nextTabs[Math.max(0, slice.tabs.findIndex((tab) => tab.id === tabId) - 1)] ??
              nextTabs[0]).id
            : slice.activeTabId;
        return {
          activeTabId: nextTabs.some((tab) => tab.id === activeTabId)
            ? activeTabId
            : nextTabs[0].id,
          nextQueryIndex: fallbackTab ? fallbackNext : slice.nextQueryIndex,
          tabs: nextTabs,
        };
      }),
    );
  },
  openQueryTab: (workspaceId, input = {}, formatQueryTitle) => {
    const fmt = formatQueryTitle ?? defaultQueryTitle;
    const index = get().byWorkspace[workspaceId]?.nextQueryIndex ?? 2;
    const tab = createQueryTab(index, input, fmt);
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...slice,
        activeTabId: tab.id,
        nextQueryIndex: slice.nextQueryIndex + 1,
        tabs: [...slice.tabs, tab],
      })),
    );
    return tab.id;
  },
  openTableTab: (workspaceId, connectionId, table, segment = "data") => {
    const tabId = databaseTableTabId(connectionId, table);
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => {
        const exists = slice.tabs.some((tab) => tab.id === tabId);
        return {
          ...slice,
          activeTabId: tabId,
          tabs: exists
            ? slice.tabs.map((tab) =>
                tab.id === tabId && tab.kind === "table" ? { ...tab, segment } : tab,
              )
            : [...slice.tabs, createTableTab(tabId, connectionId, table, segment)],
        };
      }),
    );
    return tabId;
  },
  removeConnectionTabs: (workspaceId, connectionId, formatQueryTitle) => {
    const fmt = formatQueryTitle ?? defaultQueryTitle;
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => {
        const needsFallback =
          slice.tabs.filter((tab) => !(tab.kind === "table" && tab.connectionId === connectionId))
            .length === 0;
        const { index: fallbackIndex, next: fallbackNext } = needsFallback
          ? allocateNextQueryIndex(slice.nextQueryIndex)
          : { index: slice.nextQueryIndex, next: slice.nextQueryIndex };
        const fallbackTab = needsFallback ? createQueryTab(fallbackIndex, {}, fmt) : null;
        const tabs = slice.tabs
          .filter((tab) => !(tab.kind === "table" && tab.connectionId === connectionId))
          .map((tab) =>
            tab.kind === "query" && tab.connectionId === connectionId
              ? {
                  ...tab,
                  catalog: null,
                  connectionId: null,
                  error: null,
                  pendingConfirmation: false,
                  schema: null,
                }
              : tab,
          );
        const nextTabs = tabs.length
          ? tabs
          : fallbackTab
            ? [fallbackTab]
            : [createQueryTab(slice.nextQueryIndex, {}, fmt)];
        return {
          activeTabId: nextTabs.some((tab) => tab.id === slice.activeTabId)
            ? slice.activeTabId
            : nextTabs[0].id,
          nextQueryIndex: fallbackTab ? fallbackNext : slice.nextQueryIndex,
          tabs: nextTabs,
        };
      }),
    );
  },
  reorderTabs: (workspaceId, fromIndex, toIndex) => {
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => {
        if (
          fromIndex < 0 ||
          toIndex < 0 ||
          fromIndex >= slice.tabs.length ||
          toIndex >= slice.tabs.length ||
          fromIndex === toIndex
        ) {
          return slice;
        }
        const tabs = [...slice.tabs];
        const [moved] = tabs.splice(fromIndex, 1);
        tabs.splice(toIndex, 0, moved);
        return { ...slice, tabs };
      }),
    );
  },
  setActiveTabId: (workspaceId, tabId) => {
    set((state) =>
      withWorkspace(state, workspaceId, (slice) =>
        slice.tabs.some((tab) => tab.id === tabId) ? { ...slice, activeTabId: tabId } : slice,
      ),
    );
  },
  updateQueryTab: (workspaceId, tabId, patch) => {
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...slice,
        tabs: slice.tabs.map((tab) => {
          if (tab.id !== tabId || tab.kind !== "query") {
            return tab;
          }
          const nextPatch = typeof patch === "function" ? patch(tab) : patch;
          return { ...tab, ...nextPatch };
        }),
      })),
    );
  },
  updateTableTab: (workspaceId, tabId, patch) => {
    set((state) =>
      withWorkspace(state, workspaceId, (slice) => ({
        ...slice,
        tabs: slice.tabs.map((tab) => {
          if (tab.id !== tabId || tab.kind !== "table") {
            return tab;
          }
          const nextPatch = typeof patch === "function" ? patch(tab) : patch;
          return { ...tab, ...nextPatch };
        }),
      })),
    );
  },
}));

export function resetDatabaseTabStore(workspaceId?: string) {
  if (workspaceId === undefined) {
    useDatabaseTabStore.setState({ byWorkspace: {} });
    return;
  }
  useDatabaseTabStore.setState((state) => {
    const next = { ...state.byWorkspace };
    delete next[workspaceId];
    return { byWorkspace: next };
  });
}

export { DEFAULT_WORKSPACE_TAB_STATE };
