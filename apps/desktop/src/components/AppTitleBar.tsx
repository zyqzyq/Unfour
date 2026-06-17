import {
  Activity,
  MoreHorizontal,
  Settings,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Workspace } from "@unfour/command-client";
import { GlobalToolbar, IconButton } from "@unfour/ui";
import { isTauriRuntime } from "./module-helpers";
import { WindowControls } from "./WindowControls";
import { WorkspaceMenu } from "./WorkspaceMenu";

export function AppTitleBar({
  activeWorkspace,
  healthReady,
  onActivateWorkspace,
  onOpenCommandPalette,
  syncStrategy,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  healthReady: boolean;
  onActivateWorkspace: (workspaceId: string) => void;
  onOpenCommandPalette: () => void;
  onToggleBottomPanel: () => void;
  onToggleInspector: () => void;
  syncStrategy: string;
  workspaces: Workspace[];
}) {
  async function dragWindow(event: React.MouseEvent<HTMLDivElement>) {
    if (event.button !== 0 || !isTauriRuntime()) {
      return;
    }

    await getCurrentWindow().startDragging();
  }

  return (
    <GlobalToolbar
      center={<div className="h-full min-w-0 flex-1" />}
      className="bg-[var(--u-color-surface-subtle)]"
      left={
        <>
          <div className="flex h-[26px] w-[26px] items-center justify-center rounded-[var(--u-radius-sm)] bg-[var(--u-color-primary)] text-[var(--u-color-primary-foreground)]">
            <Activity size={15} />
          </div>
          <span className="mr-3 text-[13px] font-semibold text-[var(--u-color-text)]">
            Unfour
          </span>
          <div className="mx-1 h-5 w-px bg-[var(--u-color-border)]" />
          <WorkspaceMenu
            activeWorkspace={activeWorkspace}
            className="ml-1"
            onActivateWorkspace={onActivateWorkspace}
            workspaces={workspaces}
          />
        </>
      }
      onDragRegionMouseDown={dragWindow}
      right={
        <>
          <span
            className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] px-2 py-0.5 font-mono text-[11px] text-[var(--u-color-text-muted)]"
            title={`${healthReady ? "Storage ready" : "Checking storage"} · ${syncStrategy}`}
          >
            v0.1.0
          </span>
          <button
            className="flex h-7 w-7 items-center justify-center rounded-full bg-[var(--u-color-surface-muted)] text-[11px] font-semibold text-[var(--u-color-text)]"
            type="button"
          >
            UF
          </button>
          <IconButton label="Settings" onClick={onOpenCommandPalette}>
            <Settings size={15} />
          </IconButton>
          <IconButton label="More actions" onClick={onOpenCommandPalette}>
            <MoreHorizontal size={16} />
          </IconButton>
          <WindowControls />
        </>
      }
    />
  );
}
