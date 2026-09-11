import type { DatabaseStatementResult } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

export function SqlBatchMessages({ statements }: { statements: DatabaseStatementResult[] }) {
  const { t } = useI18n();
  return (
    <div className="min-h-0 flex-1 space-y-3 overflow-auto p-2 text-[12px]">
      {statements.map((entry) => (
        <div key={entry.index} className="space-y-1">
          <div>{t("database.batch.statement", { index: entry.index, status: t(`database.batch.${entry.status}`) })}</div>
          <pre className="whitespace-pre-wrap break-words text-[var(--u-color-text-muted)]">{entry.sql}</pre>
          {entry.result ? <div>{t("database.result.affectedRows", { rows: entry.result.affectedRows, durationMs: entry.result.durationMs })}</div> : null}
          {entry.error ? <DatabaseErrorDetails error={entry.error} /> : null}
          {entry.status === "skipped" ? <div>{t("database.batch.skippedDetail")}</div> : null}
        </div>
      ))}
    </div>
  );
}
