import { Plus, Workflow } from "lucide-react";
import { Button, EmptyState, ErrorState, LoadingState, useI18n } from "@unfour/ui";

export function FlowStart({ pending, failed, hasFlows, refreshing, onRetry, onCreate }: {
  pending: boolean;
  failed: boolean;
  hasFlows: boolean;
  refreshing: boolean;
  onRetry: () => void;
  onCreate: () => void;
}) {
  const { t } = useI18n();
  const surface = "h-full min-h-0 rounded-none border-0";
  if (pending) return <LoadingState className={surface}>{t("flow.loading")}</LoadingState>;
  if (failed) return <ErrorState className={surface}>
    <div role="alert" className="space-y-3">
      <p>{t("flow.loadFailed")}</p>
      <Button size="sm" variant="secondary" disabled={refreshing} onClick={onRetry}>{t("flow.retry")}</Button>
    </div>
  </ErrorState>;
  return <EmptyState className={surface}>
    <div className="max-w-sm space-y-3">
      <Workflow aria-hidden="true" className="mx-auto text-[var(--u-color-text-soft)]" size={24} />
      <div className="space-y-1">
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">{t(hasFlows ? "flow.start.selectTitle" : "flow.start.title")}</h2>
        <p className="leading-5">{t(hasFlows ? "flow.selectFlow" : "flow.start.description")}</p>
      </div>
      <Button size="sm" variant={hasFlows ? "secondary" : "default"} onClick={onCreate}>
        <Plus aria-hidden="true" size={14} />{t("flow.start.create")}
      </Button>
    </div>
  </EmptyState>;
}
