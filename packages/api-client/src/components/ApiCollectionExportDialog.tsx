import { useMutation } from "@tanstack/react-query";
import {
  exportApiCollection,
  type ApiCollection,
  type ApiCollectionExportFormat,
} from "@unfour/command-client";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  useFeedback,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";

export function ApiCollectionExportDialog({
  collection,
  onClose,
  workspaceId,
}: {
  collection: ApiCollection | null;
  onClose: () => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const exportMutation = useMutation({
    mutationFn: (format: ApiCollectionExportFormat) =>
      exportApiCollection(workspaceId, collection?.id ?? "", format),
    onSuccess: (result, format) => {
      if (result.saved) {
        feedback.success(
          t("api.collection.exportSuccess", { format: format.toUpperCase() }),
        );
      }
      onClose();
    },
    onError: (error) =>
      handleError(error, { key: "feedback.api.collectionExportFailed" }),
  });

  function exportAs(format: ApiCollectionExportFormat) {
    if (collection && !exportMutation.isPending) {
      exportMutation.mutate(format);
    }
  }

  return (
    <Dialog
      onOpenChange={(open) => !open && !exportMutation.isPending && onClose()}
      open={collection !== null}
    >
      <DialogContent title={t("api.collection.exportTitle")}>
        <DialogHeader>
          <DialogTitle>{t("api.collection.exportTitle")}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <DialogDescription>
            {t("api.collection.exportDescription", { name: collection?.name ?? "" })}
          </DialogDescription>
        </DialogBody>
        <DialogFooter>
          <Button
            disabled={exportMutation.isPending}
            onClick={onClose}
            type="button"
            variant="ghost"
          >
            {t("api.save.cancel")}
          </Button>
          <Button
            disabled={exportMutation.isPending || !collection}
            onClick={() => exportAs("json")}
            type="button"
            variant="secondary"
          >
            {t("api.collection.exportJson")}
          </Button>
          <Button
            disabled={exportMutation.isPending || !collection}
            onClick={() => exportAs("yaml")}
            type="button"
          >
            {t("api.collection.exportYaml")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
