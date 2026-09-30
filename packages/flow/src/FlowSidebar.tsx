import type { FlowDefinition } from "@unfour/command-client";
import { Button, SidebarHeader, SidebarRow, useI18n } from "@unfour/ui";
import { newStep } from "./model";

export function FlowSidebar({ workspaceId, definitions, pending, failed, retry, selectedFlowId, select }: {
  workspaceId: string; definitions?: FlowDefinition[]; pending: boolean; failed: boolean; retry: () => void;
  selectedFlowId?: string;
  select: (flow: FlowDefinition) => void;
}) {
  const { t } = useI18n();
  return (
    <>
        <SidebarHeader>{t("flow.title")}</SidebarHeader>
        <Button
          className="m-2"
          variant="secondary"
          size="sm"
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
          {t("flow.new")}
        </Button>
        {pending && <p className="p-2 text-xs">{t("flow.loading")}</p>}
        {failed && (
          <p role="alert" className="p-2">
            {t("flow.loadFailed")} <Button size="sm" variant="secondary" onClick={() => void retry()}>{t("flow.retry")}</Button>
          </p>
        )}
        {definitions?.map((flow) => (
          <SidebarRow
            key={flow.id}
            active={selectedFlowId === flow.id}
            onClick={() => select(flow)}
          >
            {flow.name}
          </SidebarRow>
        ))}
    </>
  );
}
