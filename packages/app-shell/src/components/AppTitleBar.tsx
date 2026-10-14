import { Plug, Settings } from "lucide-react";
import type { Workspace, WorkspaceEnvironment } from "@unfour/command-client";
import { useState, type ReactNode } from "react";
import {
  GlobalToolbar,
  ActiveEnvironmentSelect,
  IconButton,
  Button,
  useI18n,
} from "@unfour/ui";
import { SettingsDialog } from "./settings/SettingsDialog";
import { WindowControls } from "./WindowControls";
import { WorkspaceMenu } from "./WorkspaceMenu";
import type {
  DesktopAppExtensionContext,
  DesktopAppSettingsSection,
  DesktopAppWorkspaceAction,
  DesktopAppWorkspaceActionsProvider,
  DesktopAppWorkspaceDecoration,
  DesktopAppWorkspaceMenuFooterAction,
} from "../extensions";

export function AppTitleBar({
  activeWorkspace,
  activeEnvironmentId = null,
  environments = [],
  endAccessory,
  extensionContext,
  onActivateWorkspace,
  onManageVariables = () => {},
  onOpenSettings,
  onOpenEnvironmentMenu = () => {},
  onSelectEnvironment = () => {},
  settingsSections,
  workspaceActions,
  workspaceDecoration,
  workspaceMenuActions,
  workspaceMenuFooterActions,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  activeEnvironmentId?: string | null;
  environments?: WorkspaceEnvironment[];
  endAccessory?: ReactNode;
  extensionContext: DesktopAppExtensionContext;
  onActivateWorkspace: (workspaceId: string) => void;
  onManageVariables?: () => void;
  onOpenSettings?: (section?: "mcp") => void;
  onOpenEnvironmentMenu?: () => void;
  onSelectEnvironment?: (environmentId: string | null) => void;
  settingsSections?: readonly DesktopAppSettingsSection[];
  workspaceActions?: readonly DesktopAppWorkspaceAction[];
  workspaceDecoration?: DesktopAppWorkspaceDecoration;
  workspaceMenuActions?: DesktopAppWorkspaceActionsProvider;
  workspaceMenuFooterActions?: readonly DesktopAppWorkspaceMenuFooterAction[];
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState("general");
  function openSettings(section?: "mcp") {
    if (onOpenSettings) return onOpenSettings(section);
    if (section) setSettingsSection(section);
    setSettingsOpen(true);
  }

  return (
    <>
      <GlobalToolbar
        left={
          <div className="flex min-w-0 items-center gap-1">
            <WorkspaceMenu
              activeWorkspace={activeWorkspace}
              decoration={workspaceDecoration}
              extensionContext={extensionContext}
              onActivateWorkspace={onActivateWorkspace}
              workspaceActionProvider={workspaceMenuActions}
              workspaceActions={workspaceActions}
              workspaceMenuFooterActions={workspaceMenuFooterActions}
              workspaces={workspaces}
            />
            <ActiveEnvironmentSelect
              description={t("variables.activeEnvironmentHelp")}
              activeEnvironmentId={activeEnvironmentId}
              environments={environments}
              onManage={onManageVariables}
              onOpenChange={(open) => open && onOpenEnvironmentMenu()}
              onSelect={onSelectEnvironment}
            />
          </div>
        }
        right={
          <>
            <Button
              aria-label={t("app.commandPalette.openMcp")}
              aria-description={t("app.titlebar.mcpPermissionsHelp")}
              onClick={() => openSettings("mcp")}
              size="sm"
              title={t("app.titlebar.mcpPermissionsHelp")}
              variant="ghost"
            >
              <Plug aria-hidden="true" size={14} />
              {t("app.settings.sections.mcp")}
            </Button>
            <IconButton label={t("app.titlebar.settings")} onClick={() => openSettings()}>
              <Settings size={15} />
            </IconButton>
            {endAccessory}
            <WindowControls />
          </>
        }
      />
      {!onOpenSettings && <SettingsDialog
        activeSection={settingsSection}
        onSectionChange={setSettingsSection}
        extensionContext={extensionContext}
        extensionSections={settingsSections}
        onOpenChange={setSettingsOpen}
        open={settingsOpen}
      />}
    </>
  );
}
