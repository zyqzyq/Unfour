import { useState } from "react";
import type { ReactNode } from "react";
import { Tabs, useI18n } from "@unfour/ui";
import { SshConnectionsPage } from "./TerminalPage";
import { SshTasksPage } from "./components/SshTasksPage";
import { useSshConnections } from "./hooks/useSshConnections";

export function TerminalPage({
  onShellSidebarChange,
  workspaceId,
}: {
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<"connections" | "tasks">("connections");
  const connectionsQuery = useSshConnections(workspaceId);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <Tabs
        activeId={activeTab}
        onSelect={(tabId) => setActiveTab(tabId as "connections" | "tasks")}
        tabs={[
          { id: "connections", title: t("ssh.homeTabs.connections"), draggable: false },
          { id: "tasks", title: t("ssh.homeTabs.tasks"), draggable: false },
        ]}
      />
      {activeTab === "connections" ? (
        <SshConnectionsPage
          onShellSidebarChange={onShellSidebarChange}
          workspaceId={workspaceId}
        />
      ) : (
        <SshTasksPage connections={connectionsQuery.data ?? []} workspaceId={workspaceId} />
      )}
    </div>
  );
}
