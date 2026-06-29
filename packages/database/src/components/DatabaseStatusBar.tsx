import type { DatabaseConnection } from "@unfour/command-client";
import { StatusBadge, useI18n } from "@unfour/ui";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

export function DatabaseStatusBar({
  connection,
  executing,
  session,
}: {
  connection: DatabaseConnection | null;
  executing: boolean;
  session?: DatabaseConnectionSessionState;
}) {
  const { t } = useI18n();
  const status: DatabaseConnectionStatus = session?.status ?? "disconnected";
  const message = executing
    ? "SQL executing"
    : session?.message
      ? session.message
      : status === "connected"
        ? "Ready"
        : "No active database session";

  return (
    <div className="flex h-[var(--u-size-statusbar)] items-center justify-between border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-muted)] px-2 text-[12px] text-[var(--u-color-text-muted)]">
      <span className="flex min-w-0 items-center gap-2">
        <span className="truncate">{connection ? `${connection.name} · ${status}` : "No database connection"}</span>
        {connection?.readOnly ? <StatusBadge tone="warning">{t("database.fields.readOnly")}</StatusBadge> : null}
      </span>
      <span className="truncate">{message}</span>
    </div>
  );
}
