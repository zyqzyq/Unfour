import type { DatabaseConnection } from "@unfour/command-client";
import type { ReactNode } from "react";
import { ConnectionStatus, StatusBadge, StatusBar, useI18n } from "@unfour/ui";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

export function DatabaseStatusBar({
  connection,
  executing,
  rightAccessory,
  session,
  workspaceName,
}: {
  connection: DatabaseConnection | null;
  executing: boolean;
  rightAccessory?: ReactNode;
  session?: DatabaseConnectionSessionState;
  workspaceName: string;
}) {
  const { t } = useI18n();
  const status: DatabaseConnectionStatus = session?.status ?? "disconnected";
  const message = executing
    ? t("database.connection.executingSql")
    : session?.message
      ? session.message
      : status === "connected"
        ? t("database.connection.ready")
        : t("database.connection.noActiveSession");

  return (
    <StatusBar>
      <div className="flex min-w-0 items-center gap-3">
        <span className="truncate">{workspaceName}</span>
        <span className="truncate">{connection?.name ?? t("database.connection.noConnection")}</span>
        <ConnectionStatus
          label={databaseConnectionStatusLabel(status, t)}
          pulse={status === "failed"}
          status={status === "failed" ? "error" : status}
          variant="badge"
        />
        {connection?.readOnly ? <StatusBadge tone="warning">{t("database.fields.readOnly")}</StatusBadge> : null}
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <span className="truncate">{message}</span>
        {rightAccessory}
      </div>
    </StatusBar>
  );
}

function databaseConnectionStatusLabel(
  status: DatabaseConnectionStatus,
  t: ReturnType<typeof useI18n>["t"],
) {
  if (status === "connecting") {
    return t("common.actions.connecting");
  }
  return t(`database.connection.${status}`);
}
