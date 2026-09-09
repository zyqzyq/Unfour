import type { ReactNode } from "react";
import { StatusBar, useI18n } from "@unfour/ui";
import type { Workspace, WorkspaceTab } from "@unfour/command-client";
import { AlertCircle, CheckCircle2, Circle } from "lucide-react";
import { moduleLabel } from "./module-helpers";

export function StatusBarPlaceholder({
  activeTab,
  activeWorkspace,
  healthReady,
  healthError = false,
  rightAccessory,
}: {
  activeTab: WorkspaceTab;
  activeWorkspace?: Workspace;
  healthReady?: boolean;
  healthError?: boolean;
  rightAccessory?: ReactNode;
}) {
  const { t } = useI18n();
  const unavailable = healthError || healthReady === false;

  return (
    <StatusBar>
      <div className="flex min-w-0 items-center gap-4">
        <span className="flex min-w-0 items-center gap-1.5">
          {unavailable ? (
            <AlertCircle className="shrink-0 text-[var(--u-color-danger)]" size={14} />
          ) : healthReady ? (
            <CheckCircle2 className="shrink-0" size={14} />
          ) : (
            <Circle className="shrink-0 opacity-80" size={13} />
          )}
          <span className="truncate">
            {unavailable ? t("app.status.storageUnavailable") : healthReady
              ? t("app.status.storageReady") : t("app.status.checkingStorage")}
          </span>
        </span>
        <span className="truncate opacity-90">
          {activeWorkspace?.name ?? t("app.workspace.none")}
        </span>
        <span className="opacity-90">{moduleLabel(activeTab, t)}</span>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        {rightAccessory}
      </div>
    </StatusBar>
  );
}
