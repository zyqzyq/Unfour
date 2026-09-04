import { useState } from "react";
import {
  Button,
  ConfirmDialog,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  ErrorState,
  StatusBadge,
  useI18n,
} from "@unfour/ui";
import { getSyncDiagnostics, syncErrorCode } from "./syncApi";
import type { DeadLetter, SyncDiagnostics } from "./syncTypes";
import { getCloudSyncViewState, syncErrorMessageKey, viewStateTone } from "./syncViewModel";
import { SyncConflictList } from "./SyncConflictList";
import { useCloudSync } from "./useCloudSync";

function formatRelativeTime(value: string | null, t: (key: string, params?: Record<string, string | number>) => string): string {
  if (!value) return t("cloudSync.never");
  const minutes = Math.max(0, Math.round((Date.now() - new Date(value).getTime()) / 60_000));
  if (minutes < 1) return t("cloudSync.justNow");
  if (minutes < 60) return t("cloudSync.minutesAgo", { count: minutes });
  return new Date(value).toLocaleString();
}

export function WorkspaceSyncDialog() {
  const { detailTarget } = useCloudSync();
  return <WorkspaceSyncDialogContent key={detailTarget?.id ?? "closed"} />;
}

function WorkspaceSyncDialogContent() {
  const { t } = useI18n();
  const { closeDetailDialog, detailTarget, enableWorkspace, globalEnabled, pauseWorkspace, refreshNow, replaceDeadLetterWithRemote, retryDeadLetter, retryWorkspace, setServiceEnabled, statuses, workspaceErrors } = useCloudSync();
  const [busy, setBusy] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<SyncDiagnostics | null>(null);
  const [remoteConfirmation, setRemoteConfirmation] = useState<DeadLetter | null>(null);
  const status = detailTarget ? statuses.get(detailTarget.id) : undefined;
  const workspaceError = detailTarget ? workspaceErrors.get(detailTarget.id) : undefined;
  const state = workspaceError ? "attention" : status ? getCloudSyncViewState(status, globalEnabled) : "local_only";
  const pending = status ? status.pendingCount + status.uncertainCount + status.inFlightCount + status.deadCount : 0;

  const run = async (operation: () => Promise<void>) => {
    setBusy(true);
    setErrorCode(null);
    try { await operation(); setDiagnostics(null); } catch (error) { setErrorCode(syncErrorCode(error)); } finally { setBusy(false); }
  };

  const loadDiagnostics = async () => {
    if (!detailTarget || diagnostics) return;
    try { setDiagnostics(await getSyncDiagnostics(detailTarget.id)); } catch (error) { setErrorCode(syncErrorCode(error)); }
  };

  const remoteTargetName = remoteConfirmation?.entityName ?? remoteConfirmation?.entityId ?? "";

  return <><Dialog onOpenChange={(open) => { if (!open && !busy) closeDetailDialog(); }} open={Boolean(detailTarget)}>
    <DialogContent title={t("cloudSync.title")}>
      <DialogHeader><DialogTitle>{t("cloudSync.title")}</DialogTitle>{!busy && <DialogXClose label={t("cloudSync.close")} />}</DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <h3 className="text-sm font-semibold">{detailTarget?.name}</h3>
        {(errorCode ?? workspaceError) && <ErrorState className="min-h-0 items-start justify-start text-left">{t(syncErrorMessageKey((errorCode ?? workspaceError)!))}</ErrorState>}
        <dl className="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-2 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] p-3 text-xs">
          <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.detail.status")}</dt><dd><StatusBadge tone={viewStateTone(state)}>{state === "offline" ? t("cloudSync.detail.waitingConnection") : t(`cloudSync.status.${state}`)}</StatusBadge></dd>
          <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.detail.lastSynced")}</dt><dd>{formatRelativeTime(status?.binding?.lastSuccessAt ?? null, t)}</dd>
          {pending > 0 && <><dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.pending")}</dt><dd>{t("cloudSync.detail.changesPending", { count: pending })}</dd></>}
          {status && status.deadCount > 0 && <><dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.deadLetter.count")}</dt><dd>{status.deadCount}</dd></>}
        </dl>
        {state === "offline" && <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.detail.offlineDescription")}</p>}
        {state === "auth_required" && <p className="text-xs text-[var(--u-color-danger)]">{t("cloudSync.detail.authRequiredDescription")}</p>}
        {state === "capability_required" && <p className="text-xs text-[var(--u-color-warning)]">{t("cloudSync.detail.capabilityRequiredDescription")}</p>}
        {state === "attention" && <><p className="text-xs text-[var(--u-color-warning)]">{t("cloudSync.detail.attentionDescription")}</p>{status && status.conflictCount > 0 && detailTarget && <SyncConflictList onResolved={() => void refreshNow()} workspaceId={detailTarget.id} />}</>}
        {status && status.deadLetters.length > 0 && detailTarget && <section aria-label={t("cloudSync.deadLetter.title")} className="flex flex-col gap-2">
          <div>
            <h4 className="text-xs font-semibold">{t("cloudSync.deadLetter.title")}</h4>
            <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.deadLetter.description")}</p>
          </div>
          <ul className="flex flex-col gap-2">
            {status.deadLetters.map((entry) => <li className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] p-3 text-xs" key={entry.operationId}>
              <div className="min-w-0">
                <div className="truncate font-semibold">{entry.entityName ?? entry.entityId}</div>
                <div className="text-[var(--u-color-text-muted)]">{t(`cloudSync.deadLetter.entityType.${entry.entityType}`)}</div>
                <p className="mt-2 text-[var(--u-color-danger)]">{t(syncErrorMessageKey(entry.errorCode))}</p>
                <details className="mt-2">
                  <summary className="cursor-pointer text-[var(--u-color-text-muted)]">{t("cloudSync.technicalDetails")}</summary>
                  <code className="mt-1 block break-all text-[11px] text-[var(--u-color-text-soft)]">{entry.errorCode}</code>
                </details>
              </div>
              <div className="mt-2 flex flex-wrap justify-end gap-2">
                <Button disabled={busy} onClick={() => void run(() => retryDeadLetter(detailTarget.id, entry.operationId))} size="sm" type="button" variant="outline">{t("cloudSync.deadLetter.retryCurrentLocal")}</Button>
                <Button disabled={busy} onClick={() => { setErrorCode(null); setRemoteConfirmation(entry); }} size="sm" type="button" variant="danger">{t("cloudSync.deadLetter.useRemote")}</Button>
              </div>
            </li>)}
          </ul>
        </section>}
        <p className="text-xs text-[var(--u-color-text-muted)]">{t("cloudSync.secretPolicy")}</p>
        <details onToggle={(event) => { if (event.currentTarget.open) void loadDiagnostics(); }}>
          <summary className="cursor-pointer text-xs font-semibold">{t("cloudSync.advancedDiagnostics")}</summary>
          {diagnostics && <dl className="mt-2 grid grid-cols-[9rem_1fr] gap-x-3 gap-y-1 text-xs">
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.localWorkspaceId")}</dt><dd className="break-all font-mono">{diagnostics.localWorkspaceId}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.remoteWorkspaceId")}</dt><dd className="break-all font-mono">{diagnostics.remoteWorkspaceId}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.pullCursor")}</dt><dd>{diagnostics.pullCursor}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.pendingOutboxCount")}</dt><dd>{diagnostics.pendingOutboxCount}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.deadOutboxCount")}</dt><dd>{diagnostics.deadOutboxCount}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastErrorCode")}</dt><dd className="font-mono">{diagnostics.lastErrorCode ?? "—"}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastPush")}</dt><dd>{formatRelativeTime(diagnostics.lastPushAt, t)}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastPull")}</dt><dd>{formatRelativeTime(diagnostics.lastPullAt, t)}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.consecutiveFailures")}</dt><dd>{diagnostics.consecutiveFailureCount}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.nextRetry")}</dt><dd>{diagnostics.nextRetryAt ? new Date(diagnostics.nextRetryAt).toLocaleString() : "—"}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastServerErrorCode")}</dt><dd className="break-all font-mono">{diagnostics.lastServerErrorCode ?? "—"}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastServerRequestId")}</dt><dd className="break-all font-mono">{diagnostics.lastServerRequestId ?? "—"}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("cloudSync.lastHttpStatusPhase")}</dt><dd className="font-mono">{diagnostics.lastHttpStatus ?? "—"} / {diagnostics.lastSyncPhase ?? "—"}</dd>
          </dl>}
        </details>
      </DialogBody>
      <DialogFooter>
        {(["offline", "auth_required", "capability_required"].includes(state) || (state === "attention" && (status?.deadCount ?? 0) === 0 && (status?.conflictCount ?? 0) === 0)) && detailTarget && <Button disabled={busy} onClick={() => void run(() => retryWorkspace(detailTarget.id))} size="sm" type="button">{t("cloudSync.retry")}</Button>}
        {state !== "local_only" && detailTarget && <Button disabled={busy} onClick={() => void run(() => state === "paused" ? (globalEnabled ? enableWorkspace(detailTarget.id) : setServiceEnabled(true)) : pauseWorkspace(detailTarget.id))} size="sm" type="button" variant="outline">{state === "paused" ? t("cloudSync.resume") : t("cloudSync.pause")}</Button>}
        <Button disabled={busy} onClick={closeDetailDialog} size="sm" type="button" variant="ghost">{t("cloudSync.close")}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
  <ConfirmDialog
    confirmLabel={t("cloudSync.deadLetter.confirmUseRemote")}
    description={<>{t("cloudSync.deadLetter.confirmDescription", { name: remoteTargetName })}{errorCode && <span className="mt-2 block text-[var(--u-color-danger)]">{t(syncErrorMessageKey(errorCode))}</span>}</>}
    onConfirm={() => {
      if (!detailTarget || !remoteConfirmation) return;
      void run(async () => {
        await replaceDeadLetterWithRemote(detailTarget.id, remoteConfirmation.operationId);
        setRemoteConfirmation(null);
      });
    }}
    onOpenChange={(open) => { if (!open && !busy) setRemoteConfirmation(null); }}
    open={Boolean(remoteConfirmation)}
    pending={busy}
    title={t("cloudSync.deadLetter.confirmTitle", { name: remoteTargetName })}
    tone="danger"
  />
  </>;
}
