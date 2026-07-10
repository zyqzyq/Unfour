import { Settings } from "lucide-react";
import type { Workspace } from "@unfour/command-client";
import { useState, type ReactNode } from "react";
import {
  GlobalToolbar,
  IconButton,
  useI18n,
} from "@unfour/ui";
import { SettingsDialog } from "./settings/SettingsDialog";
import { WindowControls } from "./WindowControls";
import { WorkspaceMenu } from "./WorkspaceMenu";
import type {
  DesktopAppExtensionContext,
  DesktopAppSettingsSection,
} from "../extensions";

export function AppTitleBar({
  activeWorkspace,
  endAccessory,
  extensionContext,
  onActivateWorkspace,
  settingsSections,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  endAccessory?: ReactNode;
  extensionContext: DesktopAppExtensionContext;
  onActivateWorkspace: (workspaceId: string) => void;
  settingsSections?: readonly DesktopAppSettingsSection[];
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const [settingsOpen, setSettingsOpen] = useState(false);

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
        right={
          <>
            <IconButton label={t("app.titlebar.settings")} onClick={() => setSettingsOpen(true)}>
              <Settings size={15} />
            </IconButton>
            {endAccessory}
            <WindowControls />
          </>
        }
      />
      <SettingsDialog
        extensionContext={extensionContext}
        extensionSections={settingsSections}
        onOpenChange={setSettingsOpen}
        open={settingsOpen}
      />
    </>
  );
}
