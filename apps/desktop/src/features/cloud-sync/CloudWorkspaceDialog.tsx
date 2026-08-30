import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

export function CloudWorkspaceDialog(props: DesktopAppExtensionContext) {
  const { cloudWorkspaceDialogOpen } = useCloudSync();
  return cloudWorkspaceDialogOpen ? <OpenCloudWorkspaceDialog {...props} /> : null;
}

function OpenCloudWorkspaceDialog({ activateWorkspace, refreshWorkspaces }: DesktopAppExtensionContext) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const { cloudWorkspaceDialogOpen, closeCloudWorkspaceDialog, refreshNow, statuses } = useCloudSync();
  const [items, setItems] = useState<CloudWorkspace[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const requestId = useRef(0);
  const inFlight = useRef<Promise<CloudWorkspace[]> | null>(null);
  const mappedIds = useMemo(() => new Set([...statuses.values()]
    .map((status) => status.binding?.cloudWorkspaceId)
    .filter((id): id is string => Boolean(id))), [statuses]);

  const load = useCallback(() => {
    const currentRequest = ++requestId.current;
    // StrictMode setup/cleanup/setup shares the same read, but only the latest
    // effect subscription may publish its result.
    const request = inFlight.current ?? listCloudWorkspaces();
    inFlight.current = request;
    return request.then((workspaces) => {
      if (currentRequest !== requestId.current) return;
      setItems(workspaces);
      setErrorCode(null);
    }).catch((error: unknown) => {
      if (currentRequest === requestId.current) setErrorCode(syncErrorCode(error));
    }).finally(() => {
      if (inFlight.current === request) inFlight.current = null;
      if (currentRequest === requestId.current) setLoading(false);
    });
  }, []);

  useEffect(() => {
    void load();
    return () => { requestId.current += 1; };
  }, [load]);
  // Local status refreshes change the filter, not the remote request lifecycle.
  const visibleItems = items.filter((item) => !mappedIds.has(item.cloudWorkspaceId));

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
          {!loading && errorCode && <Button onClick={() => { setLoading(true); setErrorCode(null); void load(); }} size="sm" type="button" variant="outline">{t("cloudSync.retry")}</Button>}
          {!loading && !errorCode && visibleItems.length === 0 && <EmptyState>{t("cloudSync.cloudDialog.empty")}</EmptyState>}
          {!loading && !errorCode && visibleItems.length > 0 && <div className="divide-y divide-[var(--u-color-border)] border-y border-[var(--u-color-border)]">
            {visibleItems.map((workspace) => <div className="flex items-center justify-between gap-3 py-2" key={workspace.cloudWorkspaceId}>
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
