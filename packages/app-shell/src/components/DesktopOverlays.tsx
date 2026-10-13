import { ConfirmDialog, useI18n } from "@unfour/ui";
import type { DesktopAppExtensionContext, DesktopAppExtensions } from "../extensions";
import { DesktopCommandPalette } from "./DesktopCommandPalette";
import type { DesktopNavigationController } from "./useDesktopNavigation";

export function DesktopOverlays({ extensions, extensionContext, navigation, commandPaletteOpen, onCommandPaletteOpenChange, onOpenSettings }: {
  onOpenSettings: (section?: "mcp") => void;
  extensions?: DesktopAppExtensions;
  extensionContext: DesktopAppExtensionContext;
  navigation: DesktopNavigationController;
  commandPaletteOpen: boolean;
  onCommandPaletteOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const Overlays = extensions?.overlays;
  return <>
    <ConfirmDialog
      confirmLabel={t("variables.discard")}
      description={t("variables.discardChangesDescription")}
      onConfirm={navigation.confirmVariableManagerLeave}
      onOpenChange={(open) => !open && navigation.cancelVariableManagerLeave()}
      open={navigation.pendingVariableManagerLeave !== null}
      title={t("variables.discardChangesTitle")}
    />
    <DesktopCommandPalette
      onOpenSettings={onOpenSettings}
      extensionContext={extensionContext}
      extensionActions={extensions?.commandPaletteActions}
      onClose={() => onCommandPaletteOpenChange(false)}
      onOpen={() => onCommandPaletteOpenChange(true)}
      onSelectModule={navigation.handleSelectModule}
      onManageVariables={extensionContext.activeWorkspace ? navigation.handleManageVariables : undefined}
      open={commandPaletteOpen}
    />
    {Overlays && <Overlays {...extensionContext} />}
  </>;
}
