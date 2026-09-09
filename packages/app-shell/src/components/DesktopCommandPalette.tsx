import { useEffect } from "react";
import { CommandPalette, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import { exportDiagnosticsBundle, openDiagnosticsDir, openLogDir } from "@unfour/command-client";
import type { DesktopAppCommandPaletteAction, DesktopAppExtensionContext } from "../extensions";

export function DesktopCommandPalette({
  extensionActions = [],
  extensionContext,
  onClose,
  onManageVariables,
  onOpen,
  onSelectModule,
  open,
}: {
  extensionActions?: readonly DesktopAppCommandPaletteAction[];
  extensionContext: DesktopAppExtensionContext;
  onClose: () => void;
  onManageVariables?: () => void;
  onOpen: () => void;
  onSelectModule: (tabId: string) => void;
  open: boolean;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.isComposing || event.altKey ||
        !(event.ctrlKey || event.metaKey) || !event.shiftKey || event.key.toLowerCase() !== "p") return;
      // Do not place another modal over a connection form or confirmation.
      if (!open && document.querySelector('[role="dialog"], [role="alertdialog"]')) return;
      event.preventDefault();
      onOpen();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onOpen, open]);

  function run(action: () => unknown) {
    onClose();
    void Promise.resolve().then(action).catch((error) =>
      handleError(error, { key: "feedback.command.actionFailed" }),
    );
  }

  const actions = [
    { id: "api", label: t("app.commandPalette.openApiClient"), run: () => onSelectModule("api-main") },
    { id: "ssh", label: t("app.commandPalette.openSshTerminal"), run: () => onSelectModule("ssh-main") },
    { id: "database", label: t("app.commandPalette.openDatabase"), run: () => onSelectModule("database-main") },
    ...(onManageVariables ? [{ id: "variables", label: t("variables.manage"), run: onManageVariables }] : []),
    { id: "logs", label: t("app.commandPalette.openLogDir"), run: openLogDir },
    { id: "diagnostics", label: t("app.commandPalette.openDiagnosticsDir"), run: openDiagnosticsDir },
    { id: "export", label: t("app.commandPalette.exportDiagnosticsBundle"), run: exportDiagnosticsBundle },
  ];

  return (
    <CommandPalette
      items={[
        ...actions.map((action) => ({
          id: `shell:${action.id}`, label: action.label, onSelect: () => run(action.run),
        })),
        ...extensionActions.map((action) => ({
          id: `extension:${action.id}`, label: action.label, searchText: action.searchText,
          onSelect: () => run(() => action.run(extensionContext)),
        })),
      ]}
      onClose={onClose}
      open={open}
    />
  );
}
