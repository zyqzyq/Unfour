import { Database } from "lucide-react";
import type { ReactNode } from "react";
import type { WorkspaceTab } from "@unfour/command-client";
import {
  Sidebar,
  SidebarRow,
  SidebarSection,
  useI18n,
} from "@unfour/ui";

export function ModuleSidebar({
  activeTab,
  activeTabId,
  apiSidebarContent,
  collapsed,
  onWidthChange,
  selectedDatabaseConnectionId,
  setActiveTab,
  sshSidebarContent,
  width,
}: {
  activeTab: WorkspaceTab;
  activeTabId: string;
  apiSidebarContent?: ReactNode;
  collapsed: boolean;
  onWidthChange: (width: number) => void;
  selectedDatabaseConnectionId: string | null;
  setActiveTab: (tabId: string) => void;
  sshSidebarContent?: ReactNode;
  width: number;
}) {
  const { t } = useI18n();

  if (collapsed) {
    return null;
  }

  return (
    <Sidebar
      contentClassName={activeTab.kind === "api" ? "overflow-hidden p-0" : undefined}
      onWidthChange={onWidthChange}
      resizable
      width={width}
    >
      {activeTab.kind === "api" && apiSidebarContent}
      {activeTab.kind === "ssh" && sshSidebarContent}
      {activeTab.kind === "database" && (
        <SidebarSection title={t("app.nav.database")}>
          <SidebarRow
            active={activeTabId === "database-main" && !selectedDatabaseConnectionId}
            onClick={() => setActiveTab("database-main")}
          >
            <Database size={14} />
            <span className="truncate">{t("app.sidebar.sqlWorkspace")}</span>
          </SidebarRow>
        </SidebarSection>
      )}
    </Sidebar>
  );
}
