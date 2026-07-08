import { create } from "zustand";
import type { WorkspaceLayout, WorkspaceTab } from "@unfour/command-client";

type WorkspaceStore = {
  activeWorkspaceId?: string;
  activeTabId: string;
  layoutWorkspaceId?: string;
  selectedApiRequestId: string | null;
  selectedDatabaseConnectionId: string | null;
  selectedSshConnectionId: string | null;
  sidebarCollapsed: boolean;
  sidebarWidth: number;
  bottomPanelHeight: number;
  rightInspectorWidth: number;
  tabs: WorkspaceTab[];
  hydrateLayout: (layout: WorkspaceLayout) => void;
  openTab: (tab: WorkspaceTab) => void;
  snapshotLayout: (workspaceId: string) => WorkspaceLayout;
  setSelectedApiRequest: (requestId: string | null) => void;
  setSelectedDatabaseConnection: (connectionId: string | null) => void;
  setSelectedSshConnection: (connectionId: string | null) => void;
  setActiveTab: (tabId: string) => void;
  setActiveWorkspace: (workspaceId: string) => void;
  setSidebarWidth: (width: number) => void;
  setBottomPanelHeight: (height: number) => void;
  setRightInspectorWidth: (width: number) => void;
  toggleSidebar: () => void;
};

const initialTabs: WorkspaceTab[] = [
  { id: "api-main", title: "API Client", kind: "api" },
  { id: "ssh-main", title: "SSH Terminal", kind: "ssh" },
  { id: "database-main", title: "Database", kind: "database" },
];

const DEFAULT_SIDEBAR_WIDTH = 248;
const DEFAULT_BOTTOM_PANEL_HEIGHT = 220;
const DEFAULT_RIGHT_INSPECTOR_WIDTH = 300;

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => ({
  activeTabId: "api-main",
  selectedApiRequestId: null,
  selectedDatabaseConnectionId: null,
  selectedSshConnectionId: null,
  sidebarCollapsed: false,
  sidebarWidth: DEFAULT_SIDEBAR_WIDTH,
  bottomPanelHeight: DEFAULT_BOTTOM_PANEL_HEIGHT,
  rightInspectorWidth: DEFAULT_RIGHT_INSPECTOR_WIDTH,
  tabs: initialTabs,
  hydrateLayout: (layout) =>
    set({
      activeTabId: layout.activeTabId,
      layoutWorkspaceId: layout.workspaceId,
      selectedApiRequestId: layout.selectedApiRequestId,
      selectedDatabaseConnectionId: layout.selectedDatabaseConnectionId,
      selectedSshConnectionId: layout.selectedSshConnectionId,
      sidebarCollapsed: layout.sidebarCollapsed,
      sidebarWidth: layout.sidebarWidth ?? DEFAULT_SIDEBAR_WIDTH,
      bottomPanelHeight: layout.bottomPanelHeight ?? DEFAULT_BOTTOM_PANEL_HEIGHT,
      rightInspectorWidth: layout.rightInspectorWidth ?? DEFAULT_RIGHT_INSPECTOR_WIDTH,
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
      selectedApiRequestId: state.selectedApiRequestId,
      selectedDatabaseConnectionId: state.selectedDatabaseConnectionId,
      selectedSshConnectionId: state.selectedSshConnectionId,
      sidebarWidth: state.sidebarWidth,
      bottomPanelHeight: state.bottomPanelHeight,
      rightInspectorWidth: state.rightInspectorWidth,
      updatedAt: new Date().toISOString(),
    };
  },
  setSelectedApiRequest: (requestId) => set({ selectedApiRequestId: requestId }),
  setSelectedDatabaseConnection: (connectionId) =>
    set({ selectedDatabaseConnectionId: connectionId }),
  setSelectedSshConnection: (connectionId) => set({ selectedSshConnectionId: connectionId }),
  setActiveTab: (tabId) => set({ activeTabId: tabId }),
  setActiveWorkspace: (workspaceId) => set({ activeWorkspaceId: workspaceId }),
  setSidebarWidth: (width) => set({ sidebarWidth: width }),
  setBottomPanelHeight: (height) => set({ bottomPanelHeight: height }),
  setRightInspectorWidth: (width) => set({ rightInspectorWidth: width }),
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
}));
