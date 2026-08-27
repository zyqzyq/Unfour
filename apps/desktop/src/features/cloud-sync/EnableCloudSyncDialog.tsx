import { useState } from "react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  ErrorState,
  useI18n,
} from "@unfour/ui";
import { syncErrorCode } from "./syncApi";
import { syncErrorMessageKey } from "./syncViewModel";
import { useCloudSync } from "./useCloudSync";

export function EnableCloudSyncDialog() {
  const { t } = useI18n();
  const { closeEnableDialog, enableTarget, enableWorkspace } = useCloudSync();
  const [busy, setBusy] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);

  const confirm = async () => {
    if (!enableTarget || busy) return;
    setBusy(true);
    setErrorCode(null);
    try {
      await enableWorkspace(enableTarget.id);
      closeEnableDialog();
    } catch (error) {
      setErrorCode(syncErrorCode(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog onOpenChange={(open) => { if (!open && !busy) closeEnableDialog(); }} open={Boolean(enableTarget)}>
      <DialogContent title={t("cloudSync.enableDialog.title", { name: enableTarget?.name ?? "" })}>
        <DialogHeader>
          <DialogTitle>{t("cloudSync.enableDialog.title", { name: enableTarget?.name ?? "" })}</DialogTitle>
          {!busy && <DialogXClose label={t("cloudSync.close")} />}
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          {errorCode && <ErrorState>{t(syncErrorMessageKey(errorCode))}</ErrorState>}
          <div>
            <p className="text-xs font-semibold">{t("cloudSync.enableDialog.willSync")}</p>
            <ul className="mt-1 list-disc space-y-1 pl-5 text-xs">
              <li>{t("cloudSync.scope.workspace")}</li>
              <li>{t("cloudSync.scope.connections")}</li>
              <li>{t("cloudSync.scope.environments")}</li>
              <li>{t("cloudSync.scope.variables")}</li>
              <li>{t("cloudSync.scope.apiCollections")}</li>
              <li>{t("cloudSync.scope.apiFolders")}</li>
              <li>{t("cloudSync.scope.apiRequests")}</li>
              <li>{t("cloudSync.scope.sshTasks")}</li>
            </ul>
          </div>
          <div>
            <p className="text-xs font-semibold">{t("cloudSync.enableDialog.willNotSync")}</p>
            <ul className="mt-1 list-disc space-y-1 pl-5 text-xs text-[var(--u-color-text-muted)]">
              <li>{t("cloudSync.scope.secrets")}</li>
              <li>{t("cloudSync.scope.ssh")}</li>
              <li>{t("cloudSync.scope.database")}</li>
              <li>{t("cloudSync.scope.historyRuntime")}</li>
            </ul>
          </div>
          <p className="rounded-[var(--u-radius-sm)] bg-[var(--u-color-warning-soft)] p-2 text-xs text-[var(--u-color-warning)]">
            {t("cloudSync.secretPolicy")}
          </p>
        </DialogBody>
        <DialogFooter>
          <Button disabled={busy} onClick={closeEnableDialog} size="sm" type="button" variant="ghost">{t("cloudSync.cancel")}</Button>
          <Button disabled={busy} onClick={() => void confirm()} size="sm" type="button">{busy ? t("cloudSync.enabling") : t("cloudSync.enableAndUpload")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
