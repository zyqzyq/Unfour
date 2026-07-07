import { AlertCircle } from "lucide-react";
import { Button, useI18n } from "@unfour/ui";

// Prominent, non-dismissable banner shown at the top of the database workspace
// when the active connection has failed (see `DatabasePage`). Unlike the tiny
// status dot, this surfaces the real error (`session.message`) and offers
// immediate recovery actions, so a failed connect is hard to miss.
export function DatabaseConnectionErrorBanner({
  connectionName,
  message,
  onRetry,
  onEdit,
}: {
  connectionName: string;
  message?: string | null;
  onRetry: () => void;
  onEdit: () => void;
}) {
  const { t } = useI18n();
  return (
    <div
      className="flex shrink-0 items-start gap-2 rounded-[var(--u-radius-sm)] border border-[color:color-mix(in_srgb,var(--u-color-danger)_34%,var(--u-color-border))] bg-[var(--u-color-danger-soft)] px-3 py-2 text-[var(--u-color-danger)]"
      role="alert"
    >
      <AlertCircle size={16} className="mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">
          {t("database.connection.failedBanner")}
          <span className="ml-1 opacity-80">{connectionName}</span>
        </p>
        {message ? (
          <p className="mt-0.5 break-words text-[12px] leading-relaxed">{message}</p>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t("common.actions.retry")}
        </Button>
        <Button size="sm" variant="outline" onClick={onEdit}>
          {t("database.tree.editConnection")}
        </Button>
      </div>
    </div>
  );
}
