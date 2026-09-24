import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Plus, Upload } from "lucide-react";
import { importApiCollection, previewApiCollectionImport } from "@unfour/command-client";
import {
  useFeedback,
  useFeedbackErrorHandler,
  useI18n,
  Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from "@unfour/ui";

export function ApiCollectionToolbarActions({
  createPending,
  onCreate,
  workspaceId,
  onImported,
}: {
  createPending: boolean;
  onCreate: () => void;
  workspaceId: string;
  onImported: (id: string) => void;
}) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<Awaited<ReturnType<typeof previewApiCollectionImport>>>(null);
  const previewMutation = useMutation({
    mutationFn: () => previewApiCollectionImport(workspaceId),
    onSuccess: setPending,
    onError: (error) => handleError(error, { key: "feedback.api.collectionImportFailed" }),
  });
  const importMutation = useMutation({
    mutationFn: () => importApiCollection(workspaceId, pending?.content ?? ""),
    onSuccess: (result) => {
      if (!result.imported || !result.collection) {
        return;
      }
      queryClient.invalidateQueries({ queryKey: ["api-collections", workspaceId] });
      queryClient.invalidateQueries({
        queryKey: ["api-collection-folders", workspaceId],
      });
      queryClient.invalidateQueries({ queryKey: ["api-saved", workspaceId] });
      onImported(result.collection.id);
      setPending(null);
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
        disabled={importMutation.isPending || previewMutation.isPending}
        onClick={() => previewMutation.mutate()}
        title={t("api.collection.import")}
        type="button"
      >
        <Upload size={14} />
      </button>
      <Dialog open={pending !== null} onOpenChange={(open) => !open && !importMutation.isPending && setPending(null)}>
        <DialogContent title={t("exchange.preview")}>
          <DialogHeader><DialogTitle>{t("exchange.preview")}</DialogTitle></DialogHeader>
          <DialogBody>
            <p>{t(`exchange.collection.${pending?.preview.format ?? "unfour"}`)} · {pending?.preview.name}</p>
            {pending?.preview.conflict && <p>{t("exchange.collectionTargetName", { name: pending.preview.targetName })}</p>}
            <p>{t("exchange.counts", { folders: pending?.preview.folderCount ?? 0, requests: pending?.preview.requestCount ?? 0, scripts: pending?.preview.scriptCount ?? 0 })}</p>
            <p>{t("exchange.variables")}: {pending?.preview.variables.join(", ") || t("exchange.none")}</p>
            <ul className="max-h-48 overflow-auto text-[12px] text-[var(--u-color-text-muted)]">
              {pending?.preview.warnings.map((warning) => <li key={warning}>{t(`exchange.warnings.${warning}`)}</li>)}
            </ul>
          </DialogBody>
          <DialogFooter>
            <Button variant="ghost" disabled={importMutation.isPending} onClick={() => setPending(null)}>{t("api.save.cancel")}</Button>
            <Button disabled={importMutation.isPending} onClick={() => importMutation.mutate()}>{t("exchange.import")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
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
