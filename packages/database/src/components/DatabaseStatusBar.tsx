import type { DatabaseConnection } from "@unfour/command-client";
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
      <span className="truncate">{connection ? `${connection.name} · ${status}` : "No database connection"}</span>
      <span className="truncate">{message}</span>
    </div>
  );
}
