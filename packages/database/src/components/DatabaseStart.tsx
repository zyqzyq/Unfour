import { Database, Plus } from "lucide-react";
import { Button, EmptyState, ErrorState, LoadingState, useI18n } from "@unfour/ui";

export function DatabaseStart({ pending, failed, refreshing, onRetry, onCreate }: {
  pending: boolean;
  failed: boolean;
  refreshing: boolean;
  onRetry: () => void;
  onCreate: () => void;
}) {
  const { t } = useI18n();
  const surface = "min-h-0 flex-1 rounded-none border-0";
  if (pending) return <LoadingState className={surface}>{t("database.start.loading")}</LoadingState>;
  if (failed) return <ErrorState className={surface}>
    <div role="alert" className="space-y-3">
      <p>{t("database.start.failed")}</p>
      <Button size="sm" variant="secondary" disabled={refreshing} onClick={onRetry}>{t("common.actions.retry")}</Button>
    </div>
  </ErrorState>;
  return <EmptyState className={surface}>
    <div className="max-w-sm space-y-3">
      <Database aria-hidden="true" className="mx-auto text-[var(--u-color-text-soft)]" size={24} />
      <div className="space-y-1">
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">{t("database.start.title")}</h2>
        <p className="leading-5">{t("database.start.description")}</p>
      </div>
      <Button size="sm" onClick={onCreate}><Plus aria-hidden="true" size={14} />{t("database.start.create")}</Button>
    </div>
  </EmptyState>;
}
