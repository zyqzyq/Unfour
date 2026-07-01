import { Button, ConnectionStatus, Toolbar, ToolbarGroup, useI18n } from "@unfour/ui";
import type { DatabaseConnectionStatus } from "../model/types";

export function DatabaseModuleToolbar({
  connectionStatus,
  onNewQuery,
  selectedConnectionName,
}: {
  onNewQuery: () => void;
  connectionStatus: DatabaseConnectionStatus;
  selectedConnectionName: string | null;
}) {
  const { t } = useI18n();
  const connectionStatusLabel = t(`database.connection.${connectionStatus}`);

  return (
    <Toolbar>
      <ToolbarGroup>
        <Button
          onClick={() => onNewQuery()}
          size="sm"
          type="button"
          variant="outline"
        >
          {t("database.actions.newQuery")}
        </Button>
      </ToolbarGroup>
      <ToolbarGroup>
        <ConnectionStatus
          label={selectedConnectionName
            ? `${selectedConnectionName} - ${connectionStatusLabel}`
            : connectionStatusLabel}
          status={connectionStatus === "failed" ? "error" : connectionStatus}
          variant="dot"
        />
      </ToolbarGroup>
    </Toolbar>
  );
}
