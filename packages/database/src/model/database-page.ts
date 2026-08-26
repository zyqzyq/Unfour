import type { ReactNode } from "react";
import type { SavedSql } from "@unfour/command-client";

export type DatabasePageProps = {
  active?: boolean;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  onShellStatusBarChange?: (statusBar: ReactNode | null) => void;
  statusBarRightAccessory?: ReactNode;
  workspaceName?: string;
  workspaceId: string;
};

export function groupSavedSqlByConnection(saved: SavedSql[]) {
  const grouped: Record<string, SavedSql[]> = {};
  for (const item of saved) {
    if (item.connectionId) {
      (grouped[item.connectionId] ??= []).push(item);
    }
  }
  return grouped;
}
