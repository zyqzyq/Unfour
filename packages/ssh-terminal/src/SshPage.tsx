import { useCallback, useState } from "react";
import type { ReactNode } from "react";
import { SshConnectionsPage } from "./TerminalPage";
import { SshTasksPage } from "./components/SshTasksPage";
import { useSshConnections } from "./hooks/useSshConnections";

export function TerminalPage({
  active = true,
  onShellSidebarChange,
  workspaceId,
}: {
  active?: boolean;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const [activeMode, setActiveMode] = useState<"connections" | "tasks">("connections");
  const connectionsQuery = useSshConnections(workspaceId, { active });
  const openConnections = useCallback(() => setActiveMode("connections"), []);
  const openTasks = useCallback(() => setActiveMode("tasks"), []);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <div
        className={
          activeMode === "connections"
            ? "flex min-h-0 min-w-0 flex-1 flex-col"
            : "hidden"
        }
      >
        <SshConnectionsPage
          active={active && activeMode === "connections"}
          onOpenTasks={openTasks}
          onShellSidebarChange={onShellSidebarChange}
          workspaceId={workspaceId}
        />
      </div>
      <div
        className={
          activeMode === "tasks" ? "flex min-h-0 min-w-0 flex-1 flex-col" : "hidden"
        }
      >
        <SshTasksPage
          active={active && activeMode === "tasks"}
          connections={connectionsQuery.data ?? []}
          key={workspaceId}
          onOpenConnections={openConnections}
          onShellSidebarChange={onShellSidebarChange}
          workspaceId={workspaceId}
        />
      </div>
    </div>
  );
}
