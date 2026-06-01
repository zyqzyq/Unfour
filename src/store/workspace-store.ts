import { create } from "zustand";
import type { WorkspaceLayout, WorkspaceTab } from "../types";

type WorkspaceStore = {
  activeWorkspaceId?: string;
  activeTabId: string;
  layoutWorkspaceId?: string;
  sidebarCollapsed: boolean;
  tabs: WorkspaceTab[];
  hydrateLayout: (layout: WorkspaceLayout) => void;
  openTab: (tab: WorkspaceTab) => void;
  snapshotLayout: (workspaceId: string) => WorkspaceLayout;
  setActiveTab: (tabId: string) => void;
  setActiveWorkspace: (workspaceId: string) => void;
  toggleSidebar: () => void;
};

const initialTabs: WorkspaceTab[] = [
  { id: "api-main", title: "API Client", kind: "api" },
  { id: "ssh-main", title: "SSH Terminal", kind: "ssh" },
  { id: "database-main", title: "Database", kind: "database" },
];

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => ({
  activeTabId: "api-main",
  sidebarCollapsed: false,
  tabs: initialTabs,
  hydrateLayout: (layout) =>
    set({
      activeTabId: layout.activeTabId,
      layoutWorkspaceId: layout.workspaceId,
      sidebarCollapsed: layout.sidebarCollapsed,
      tabs: layout.tabs.length ? layout.tabs : initialTabs,
    }),
  openTab: (tab) =>
    set((state) => ({
      tabs: state.tabs.some((item) => item.id === tab.id)
        ? state.tabs
        : [...state.tabs, tab],
      activeTabId: tab.id,
    })),
  snapshotLayout: (workspaceId): WorkspaceLayout => {
    const state = get();
    return {
      workspaceId,
      sidebarCollapsed: state.sidebarCollapsed,
      activeTabId: state.activeTabId,
      tabs: state.tabs,
      selectedApiRequestId: null,
      selectedDatabaseConnectionId: null,
      selectedSshConnectionId: null,
      updatedAt: new Date().toISOString(),
    };
  },
  setActiveTab: (tabId) => set({ activeTabId: tabId }),
  setActiveWorkspace: (workspaceId) => set({ activeWorkspaceId: workspaceId }),
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
}));
