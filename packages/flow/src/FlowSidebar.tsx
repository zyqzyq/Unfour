import type { FlowDefinition } from "@unfour/command-client";
import { Plus, Workflow } from "lucide-react";
import {
  Button,
  EmptyState,
  ErrorState,
  IconButton,
  LoadingState,
  TreeView,
  useI18n,
} from "@unfour/ui";
import { newStep } from "./model";

export function FlowSidebar({ workspaceId, definitions = [], pending, failed, busy, retry, selectedFlowId, select }: {
  workspaceId: string; definitions?: FlowDefinition[]; pending: boolean; failed: boolean; retry: () => void;
  busy: boolean;
  selectedFlowId?: string;
  select: (flow: FlowDefinition) => void;
}) {
  const { t } = useI18n();
  return (
    <section className="flex h-full min-h-0 flex-col space-y-1" aria-label={t("flow.title")} aria-busy={busy || pending}>
      <div className="flex h-7 shrink-0 items-center justify-between gap-2 px-1">
        <span className="flex items-center gap-1.5 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <Workflow size={13} />
          {t("flow.title")}
        </span>
        <IconButton
          label={t("flow.new")}
          disabled={busy}
          size="compact"
          onClick={() =>
            select({
              id: "",
              workspaceId,
              name: t("flow.new"),
              revision: 0,
              inputs: [],
              steps: [newStep("api", t("flow.api"))],
            })
          }
        >
          <Plus size={14} />
        </IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {pending && !definitions.length && <LoadingState className="min-h-[72px]">{t("flow.loading")}</LoadingState>}
        {failed && (
          <ErrorState className="min-h-[72px]">
            <div role="alert" className="space-y-2">
              <div>{t("flow.loadFailed")}</div>
              <Button size="sm" variant="secondary" onClick={retry}>{t("flow.retry")}</Button>
            </div>
          </ErrorState>
        )}
        {definitions.length ? (
          <TreeView
            items={definitions.map((flow) => ({
              id: flow.id,
              label: flow.name,
              title: flow.name,
              icon: <Workflow size={13} />,
              disabled: busy,
            }))}
            selectedId={selectedFlowId}
            onSelect={(item) => {
              const flow = definitions.find((candidate) => candidate.id === item.id);
              if (flow) select(flow);
            }}
          />
        ) : !pending && !failed && (
          <EmptyState className="min-h-[72px]">{t("flow.empty")}</EmptyState>
        )}
      </div>
    </section>
  );
}
