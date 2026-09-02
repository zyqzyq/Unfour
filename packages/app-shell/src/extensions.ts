import type {
  Workspace,
  WorkspaceEnvironmentVariable,
  WorkspaceTab,
  WorkspaceVariable,
} from "@unfour/command-client";
import type { ComponentType, ReactNode } from "react";

export type DesktopAppExtensionId = `${string}.${string}`;

export type DesktopAppExtensionContext = Readonly<{
  activeWorkspace: Readonly<Workspace> | undefined;
  activeTab: Readonly<WorkspaceTab>;
  activateWorkspace: (workspaceId: string) => void | Promise<void>;
  refreshWorkspaces: () => void | Promise<void>;
}>;

export type DesktopAppExtensionSlot = ComponentType<DesktopAppExtensionContext>;

export type DesktopAppSettingsSlot = "general" | "about";

/**
 * Settings sections without a slot add a navigation item. Slotted sections
 * render inside an existing core Settings page while keeping their feature
 * implementation in the owning Desktop module.
 */
export type DesktopAppSettingsSection =
  | Readonly<{
      id: DesktopAppExtensionId;
      label: ReactNode;
      component: DesktopAppExtensionSlot;
      slot?: undefined;
    }>
  | Readonly<{
      id: DesktopAppExtensionId;
      component: DesktopAppExtensionSlot;
      slot: DesktopAppSettingsSlot;
    }>;

export type DesktopAppCommandPaletteAction = Readonly<{
  id: DesktopAppExtensionId;
  label: ReactNode;
  run: (context: DesktopAppExtensionContext) => void | Promise<unknown>;
}>;

export type DesktopAppWorkspaceDecorationProps = DesktopAppExtensionContext &
  Readonly<{
    active: boolean;
    placement: "trigger" | "listItem";
    workspace: Readonly<Workspace>;
  }>;

export type DesktopAppWorkspaceDecoration =
  ComponentType<DesktopAppWorkspaceDecorationProps>;

export type DesktopAppWorkspaceActionContext = DesktopAppExtensionContext &
  Readonly<{ workspace: Readonly<Workspace> }>;

export type DesktopAppWorkspaceAction = Readonly<{
  id: DesktopAppExtensionId;
  label: ReactNode;
  icon?: ReactNode;
  disabled?: boolean | ((context: DesktopAppWorkspaceActionContext) => boolean);
  disabledReason?:
    | ReactNode
    | ((context: DesktopAppWorkspaceActionContext) => ReactNode);
  run: (context: DesktopAppWorkspaceActionContext) => void | Promise<unknown>;
}>;

export type DesktopAppWorkspaceActionsProvider = (
  context: DesktopAppExtensionContext,
  workspace: Readonly<Workspace>,
) => readonly DesktopAppWorkspaceAction[];

export type DesktopAppWorkspaceMenuFooterAction = Readonly<{
  id: DesktopAppExtensionId;
  label: ReactNode;
  icon?: ReactNode;
  disabled?: boolean | ((context: DesktopAppExtensionContext) => boolean);
  run: (context: DesktopAppExtensionContext) => void | Promise<unknown>;
}>;

export type DesktopAppWorkspaceVariableDecorationProps =
  DesktopAppExtensionContext &
    Readonly<{
      variable: Readonly<WorkspaceVariable | WorkspaceEnvironmentVariable>;
    }>;

export type DesktopAppWorkspaceVariableDecoration =
  ComponentType<DesktopAppWorkspaceVariableDecorationProps>;

export type DesktopAppExtensions = Readonly<{
  titleBarEnd?: DesktopAppExtensionSlot;
  statusBarEnd?: DesktopAppExtensionSlot;
  settingsSections?: readonly DesktopAppSettingsSection[];
  commandPaletteActions?: readonly DesktopAppCommandPaletteAction[];
  workspaceDecoration?: DesktopAppWorkspaceDecoration;
  workspaceMenuActions?: DesktopAppWorkspaceActionsProvider;
  workspaceMenuFooterActions?: readonly DesktopAppWorkspaceMenuFooterAction[];
  workspaceActions?: readonly DesktopAppWorkspaceAction[];
  workspaceVariableDecoration?: DesktopAppWorkspaceVariableDecoration;
  overlays?: DesktopAppExtensionSlot;
}>;
