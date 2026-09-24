import { useMutation } from "@tanstack/react-query";
import { Download, LoaderCircle } from "lucide-react";
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
          <div className="my-3 grid grid-cols-2 gap-2" aria-busy={exportMutation.isPending}>
            {([
              ["unfour", "exchange.collection.unfour"],
              ["postman", "exchange.collection.postman"],
              ["json", "api.collection.exportJson"],
              ["yaml", "api.collection.exportYaml"],
            ] as const).map(([format, label]) => (
              <Button
                key={format}
                className="h-auto min-h-10 justify-start whitespace-normal py-2 text-left"
                disabled={exportMutation.isPending || !collection}
                onClick={() => exportAs(format)}
                type="button"
                variant={format === "unfour" ? "default" : "outline"}
              >
                {exportMutation.isPending && exportMutation.variables === format
                  ? <LoaderCircle aria-hidden="true" className="shrink-0 animate-spin" size={14} />
                  : <Download aria-hidden="true" className="shrink-0" size={14} />}
                {t(label)}
              </Button>
            ))}
          </div>
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.secretExport")}</p>
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.reselectFiles")}</p>
          <p className="mt-2 text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.postmanExport")}</p>
          <p className="mt-2 text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.openapiProjection")}</p>
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
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
