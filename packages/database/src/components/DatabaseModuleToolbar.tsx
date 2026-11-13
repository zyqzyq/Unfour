import { Database, Plus } from "lucide-react";
import { Button, ConnectionStatus, Toolbar, ToolbarGroup, useI18n } from "@unfour/ui";
import type { DatabaseConnectionStatus } from "../model/types";

export function DatabaseModuleToolbar({
  connectionStatus,
  onNewQuery,
  selectedConnectionName,
  temporaryHandle,
}: {
  onNewQuery: () => void;
  connectionStatus: DatabaseConnectionStatus;
  selectedConnectionName: string | null;
  temporaryHandle?: string;
}) {
  const { t } = useI18n();
  const connectionStatusLabel = t(`database.connection.${connectionStatus}`);

  return (
    <Toolbar className="min-h-12 px-3">
      <ToolbarGroup className="gap-2.5">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-[var(--u-radius-md)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]">
          <Database aria-hidden="true" size={15} />
        </span>
        <ConnectionStatus
          label={selectedConnectionName
            ? `${selectedConnectionName} - ${connectionStatusLabel}`
            : connectionStatusLabel}
          pulse={connectionStatus === "failed"}
          status={connectionStatus === "failed" ? "error" : connectionStatus}
          variant="badge"
        />
        {temporaryHandle && <span className="text-xs text-[var(--u-color-text-muted)]">{t("network.temporarySession", { id: temporaryHandle.slice(-8) })}</span>}
      </ToolbarGroup>
      <ToolbarGroup>
        <Button
          onClick={() => onNewQuery()}
          size="sm"
          type="button"
          variant="outline"
        >
          <Plus aria-hidden="true" size={14} />
          {t("database.actions.newQuery")}
        </Button>
      </ToolbarGroup>
    </Toolbar>
  );
}
