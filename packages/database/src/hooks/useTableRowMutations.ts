import { useMutation } from "@tanstack/react-query";
import { mutateDatabaseRow } from "@unfour/command-client";
import { useFeedback, useI18n } from "@unfour/ui";
import type { DatabaseTableWorkspaceTab } from "../model/types";
import type { useDatabaseTabs } from "./useDatabaseTabs";

export function useTableRowMutations({
  activeTableTab,
  databaseTabs,
  refreshTablePage,
  workspaceId,
}: {
  activeTableTab: DatabaseTableWorkspaceTab | null;
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  refreshTablePage: () => void;
  workspaceId: string;
}) {
  const feedback = useFeedback();
  const { t } = useI18n();
  const rowMutation = useMutation({
    mutationFn: mutateDatabaseRow,
    onError: (error) => {
      if (activeTableTab) databaseTabs.updateTableTab(activeTableTab.id, { error });
    },
  });

  async function applyPendingTableChanges() {
    if (!activeTableTab?.pendingChanges.length) return;
    const tab = activeTableTab;
    let appliedAny = false;
    databaseTabs.updateTableTab(tab.id, { error: null });

    for (const change of [...tab.pendingChanges]) {
      try {
        const result = await rowMutation.mutateAsync({
          workspaceId,
          connectionId: tab.connectionId,
          catalog: tab.table.catalog,
          schema: tab.table.schema,
          tableName: tab.table.name,
          operation: change.operation,
          values: change.values,
          primaryKey: change.primaryKey,
          originalValues: change.originalValues,
          confirmMutation: true,
        });
        appliedAny = true;
        databaseTabs.updateTableTab(tab.id, (current) => ({
          pendingChanges: current.pendingChanges.filter((candidate) => candidate.id !== change.id),
        }));
        if (result.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
          feedback.info(t("feedback.activityRecordFailed"));
        }
      } catch {
        if (appliedAny) refreshTablePage();
        return;
      }
    }
    refreshTablePage();
  }

  return { applyPendingTableChanges, rowMutation };
}
