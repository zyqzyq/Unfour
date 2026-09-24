import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  exportApiCollection,
  type ApiCollection,
  type ApiCollectionExportFormat,
} from "@unfour/command-client";
import {
  Button,
  Select,
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
  const [format, setFormat] = useState<ApiCollectionExportFormat>("unfour");
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
          <Select
            aria-label={t("exchange.format")}
            value={format === "yaml" ? "json" : format}
            onChange={(event) => {
              const value = event.target.value;
              if (value === "unfour" || value === "postman" || value === "json") setFormat(value);
            }}
            options={[
              { value: "unfour", label: t("exchange.collection.unfour") },
              { value: "postman", label: t("exchange.collection.postman") },
              { value: "json", label: t("exchange.collection.openapi") },
            ]}
          />
          {(format === "json" || format === "yaml") && <Select
            aria-label={t("exchange.encoding")}
            value={format}
            onChange={(event) => setFormat(event.target.value === "yaml" ? "yaml" : "json")}
            options={[{ value: "json", label: t("api.collection.exportJson") }, { value: "yaml", label: t("api.collection.exportYaml") }]}
          />}
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.secretExport")}</p>
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.reselectFiles")}</p>
          {format !== "unfour" && <p className="text-[12px] text-[var(--u-color-text-muted)]">{t(`exchange.warnings.${format === "postman" ? "postmanExport" : "openapiProjection"}`)}</p>}
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
            onClick={() => exportAs(format)}
            type="button"

          >
            {t("exchange.export")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
