import { useCallback, useEffect, useMemo, useState } from "react";
import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  EmptyState,
  ErrorState,
  LoadingState,
  useFeedback,
  useI18n,
} from "@unfour/ui";
import { downloadCloudWorkspace, listCloudWorkspaces, syncErrorCode } from "./syncApi";
import type { CloudWorkspace } from "./syncTypes";
import { syncErrorMessageKey } from "./syncViewModel";
import { useCloudSync } from "./useCloudSync";

export function CloudWorkspaceDialog({ activateWorkspace, refreshWorkspaces }: DesktopAppExtensionContext) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const { cloudWorkspaceDialogOpen, closeCloudWorkspaceDialog, refreshNow, statuses } = useCloudSync();
  const [items, setItems] = useState<CloudWorkspace[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const mappedIds = useMemo(() => new Set([...statuses.values()]
    .map((status) => status.binding?.cloudWorkspaceId)
    .filter((id): id is string => Boolean(id))), [statuses]);

  const load = useCallback(async () => {
    setLoading(true);
    setErrorCode(null);
    try {
      setItems((await listCloudWorkspaces()).filter((item) => !mappedIds.has(item.cloudWorkspaceId)));
    } catch (error) {
      setErrorCode(syncErrorCode(error));
    } finally {
      setLoading(false);
    }
  }, [mappedIds]);

  useEffect(() => { if (cloudWorkspaceDialogOpen) void load(); }, [cloudWorkspaceDialogOpen, load]);

  const download = async (workspace: CloudWorkspace) => {
    if (busyId) return;
    setBusyId(workspace.cloudWorkspaceId);
    setErrorCode(null);
    try {
      const workspaceId = await downloadCloudWorkspace(workspace.cloudWorkspaceId, "downloadToNewWorkspace");
      await refreshWorkspaces();
      await refreshNow();
      await activateWorkspace(workspaceId);
      closeCloudWorkspaceDialog();
      feedback.success(t("cloudSync.cloudDialog.downloaded"), { description: t("cloudSync.cloudDialog.secretReminder") });
    } catch (error) {
      setErrorCode(syncErrorCode(error));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <Dialog onOpenChange={(open) => { if (!open && !busyId) closeCloudWorkspaceDialog(); }} open={cloudWorkspaceDialogOpen}>
      <DialogContent title={t("cloudSync.cloudDialog.title")}>
        <DialogHeader><DialogTitle>{t("cloudSync.cloudDialog.title")}</DialogTitle>{!busyId && <DialogXClose label={t("cloudSync.close")} />}</DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.cloudDialog.description")}</p>
          {loading && <LoadingState />}
          {!loading && errorCode && <ErrorState><span>{t(syncErrorMessageKey(errorCode))}</span></ErrorState>}
          {!loading && errorCode && <Button onClick={() => void load()} size="sm" type="button" variant="outline">{t("cloudSync.retry")}</Button>}
          {!loading && !errorCode && items.length === 0 && <EmptyState>{t("cloudSync.cloudDialog.empty")}</EmptyState>}
          {!loading && !errorCode && items.length > 0 && <div className="divide-y divide-[var(--u-color-border)] border-y border-[var(--u-color-border)]">
            {items.map((workspace) => <div className="flex items-center justify-between gap-3 py-2" key={workspace.cloudWorkspaceId}>
              <div className="min-w-0"><p className="truncate text-sm font-medium">{workspace.name ?? t("cloudSync.cloudDialog.unnamed")}</p><p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.cloudDialog.updated", { time: new Date(workspace.updatedAt).toLocaleString() })}</p></div>
              <Button disabled={Boolean(busyId)} onClick={() => void download(workspace)} size="sm" type="button">{busyId === workspace.cloudWorkspaceId ? t("cloudSync.cloudDialog.downloading") : t("cloudSync.cloudDialog.downloadAndOpen")}</Button>
            </div>)}
          </div>}
        </DialogBody>
        <DialogFooter><Button disabled={Boolean(busyId)} onClick={closeCloudWorkspaceDialog} size="sm" type="button" variant="ghost">{t("cloudSync.close")}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
