import { useCallback, useState } from "react";
import type { ReactNode } from "react";
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
  const [activeTab, setActiveTab] = useState<"connections" | "tasks">("connections");
  const connectionsQuery = useSshConnections(workspaceId);
  const openConnections = useCallback(() => setActiveTab("connections"), []);
  const openTasks = useCallback(() => setActiveTab("tasks"), []);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      {activeTab === "connections" ? (
        <SshConnectionsPage
          onOpenTasks={openTasks}
          onShellSidebarChange={onShellSidebarChange}
          workspaceId={workspaceId}
        />
      ) : (
        <SshTasksPage
          connections={connectionsQuery.data ?? []}
          key={workspaceId}
          onOpenConnections={openConnections}
          onShellSidebarChange={onShellSidebarChange}
          workspaceId={workspaceId}
        />
      )}
    </div>
  );
}
