import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus, Upload } from "lucide-react";
import { importApiCollection } from "@unfour/command-client";
import {
  useFeedback,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";

export function ApiCollectionToolbarActions({
  createPending,
  onCreate,
  workspaceId,
}: {
  createPending: boolean;
  onCreate: () => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const importMutation = useMutation({
    mutationFn: () => importApiCollection(workspaceId),
    onSuccess: (result) => {
      if (!result.imported || !result.collection) {
        return;
      }
      queryClient.invalidateQueries({ queryKey: ["api-collections", workspaceId] });
      queryClient.invalidateQueries({
        queryKey: ["api-collection-folders", workspaceId],
      });
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
      feedback.success(
        t("api.collection.importSuccess", {
          folderCount: result.folderCount,
          name: result.collection.name,
          requestCount: result.requestCount,
        }),
      );
    },
    onError: (error) =>
      handleError(error, { key: "feedback.api.collectionImportFailed" }),
  });

  return (
    <div className="flex items-center gap-0.5">
      <button
        aria-label={t("api.collection.import")}
        className="grid h-6 w-6 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] transition-colors hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
        disabled={importMutation.isPending}
        onClick={() => importMutation.mutate()}
        title={t("api.collection.import")}
        type="button"
      >
        <Upload size={14} />
      </button>
      <button
        aria-label={t("api.collection.new")}
        className="grid h-6 w-6 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] transition-colors hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
        disabled={createPending}
        onClick={onCreate}
        title={t("api.collection.new")}
        type="button"
      >
        <Plus size={14} />
      </button>
    </div>
  );
}
