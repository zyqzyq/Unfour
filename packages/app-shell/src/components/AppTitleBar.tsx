import { Settings } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Workspace } from "@unfour/command-client";
import { useState } from "react";
import {
  GlobalToolbar,
  IconButton,
  useI18n,
} from "@unfour/ui";
import { isTauriRuntime } from "./module-helpers";
import { SettingsDialog } from "./settings/SettingsDialog";
import { WindowControls } from "./WindowControls";
import { WorkspaceMenu } from "./WorkspaceMenu";

export function AppTitleBar({
  activeWorkspace,
  onActivateWorkspace,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  onActivateWorkspace: (workspaceId: string) => void;
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const [settingsOpen, setSettingsOpen] = useState(false);

  async function dragWindow(event: React.MouseEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("button,input,select,a")) {
      return;
    }
    if (event.button !== 0 || !isTauriRuntime()) {
      return;
    }

    await getCurrentWindow().startDragging();
  }

  return (
    <>
      <GlobalToolbar
        left={
          <WorkspaceMenu
            activeWorkspace={activeWorkspace}
            onActivateWorkspace={onActivateWorkspace}
            workspaces={workspaces}
          />
        }
        onDragRegionMouseDown={dragWindow}
        right={
          <>
            <IconButton label={t("app.titlebar.settings")} onClick={() => setSettingsOpen(true)}>
              <Settings size={15} />
            </IconButton>
            <WindowControls />
          </>
        }
      />
      <SettingsDialog onOpenChange={setSettingsOpen} open={settingsOpen} />
    </>
  );
}
