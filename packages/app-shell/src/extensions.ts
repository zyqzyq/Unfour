import type { Workspace, WorkspaceTab } from "@unfour/command-client";
import type { ComponentType, ReactNode } from "react";

export type DesktopAppExtensionId = `${string}.${string}`;

export type DesktopAppExtensionContext = Readonly<{
  activeWorkspace: Readonly<Workspace> | undefined;
  activeTab: Readonly<WorkspaceTab>;
}>;

export type DesktopAppExtensionSlot = ComponentType<DesktopAppExtensionContext>;

export type DesktopAppSettingsSection = Readonly<{
  id: DesktopAppExtensionId;
  label: ReactNode;
  component: DesktopAppExtensionSlot;
}>;

export type DesktopAppCommandPaletteAction = Readonly<{
  id: DesktopAppExtensionId;
  label: ReactNode;
  run: (context: DesktopAppExtensionContext) => void | Promise<unknown>;
}>;

export type DesktopAppExtensions = Readonly<{
  titleBarEnd?: DesktopAppExtensionSlot;
  statusBarEnd?: DesktopAppExtensionSlot;
  settingsSections?: readonly DesktopAppSettingsSection[];
  commandPaletteActions?: readonly DesktopAppCommandPaletteAction[];
  overlays?: DesktopAppExtensionSlot;
}>;
