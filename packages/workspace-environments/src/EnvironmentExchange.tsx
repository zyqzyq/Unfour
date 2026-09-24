import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { exportEnvironment, importEnvironment, previewEnvironmentImport, type WorkspaceEnvironment } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle, Select, useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";

export function EnvironmentExchange({ workspaceId, exportTarget, onCloseExport, onImported }: {
  workspaceId: string;
  exportTarget: WorkspaceEnvironment | null;
  onCloseExport: () => void;
  onImported: (id: string) => void;
}) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<Awaited<ReturnType<typeof previewEnvironmentImport>>>(null);
  const [format, setFormat] = useState<"unfour" | "postman">("unfour");
  const preview = useMutation({ mutationFn: () => previewEnvironmentImport(workspaceId), onSuccess: setPending,
    onError: (error) => handleError(error, { key: "exchange.failed" }), });
  const importer = useMutation({ mutationFn: () => importEnvironment(workspaceId, pending?.content ?? ""), onSuccess: (environment) => {
    void queryClient.invalidateQueries({queryKey:["workspace-environments", workspaceId]});
    setPending(null); onImported(environment.id); feedback.success(t("exchange.imported", {name:environment.name}));
  }, onError: (error) => handleError(error, {key:"exchange.failed"}) });
  const exporter = useMutation({mutationFn: () => exportEnvironment(workspaceId, exportTarget?.id ?? "", format), onSuccess: (result) => {
    if (result.saved) feedback.success(t("exchange.exported"));
    onCloseExport();
  }, onError: (error) => handleError(error, {key:"exchange.failed"})});
  return <>
    <Button variant="ghost" disabled={preview.isPending || importer.isPending} onClick={() => preview.mutate()}>{t("exchange.import")}</Button>
    <Dialog open={pending !== null} onOpenChange={(open) => !open && !importer.isPending && setPending(null)}>
      <DialogContent title={t("exchange.preview")}>
        <DialogHeader><DialogTitle>{t("exchange.preview")}</DialogTitle></DialogHeader>
        <DialogBody>
          <p>{t(`exchange.environment.${pending?.preview.format ?? "unfour"}`)} · {pending?.preview.name}</p>
          <p>{t("exchange.variables")}: {pending?.preview.variables.length ?? 0}</p>
          <ul className="max-h-48 overflow-auto text-[12px]">
            {pending?.preview.variables.map((variable) => <li key={variable.key}>{variable.key}{variable.isSecret ? ` · ${t("exchange.secret")}` : ""}{!variable.isEnabled ? ` · ${t("exchange.disabled")}` : ""}</li>)}
          </ul>
          {pending?.preview.conflict && <p>{t("exchange.conflict")}</p>}
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.environmentCopy")}</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" disabled={importer.isPending} onClick={() => setPending(null)}>{t("api.save.cancel")}</Button>
          <Button disabled={importer.isPending} onClick={() => importer.mutate()}>{t("exchange.import")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog open={exportTarget !== null} onOpenChange={(open) => !open && !exporter.isPending && onCloseExport()}>
      <DialogContent title={t("exchange.export")}>
        <DialogHeader><DialogTitle>{t("exchange.export")} · {exportTarget?.name}</DialogTitle></DialogHeader>
        <DialogBody>
          <Select aria-label={t("exchange.format")} value={format} onChange={(event) => setFormat(event.target.value === "postman" ? "postman" : "unfour")} options={[
            {value:"unfour",label:t("exchange.environment.unfour")}, {value:"postman",label:t("exchange.environment.postman")},
          ]} />
          <p className="text-[12px] text-[var(--u-color-text-muted)]">{t("exchange.warnings.secretExport")}</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" disabled={exporter.isPending} onClick={onCloseExport}>{t("api.save.cancel")}</Button>
          <Button disabled={exporter.isPending} onClick={() => exporter.mutate()}>{t("exchange.export")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </>;
}
