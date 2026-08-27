import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  useI18n,
} from "@unfour/ui";
import type { UpdateRecovery } from "./updateTypes";
import { useUpdate } from "./useUpdate";

function Row({ label, value }: { label: string; value: string }) {
  return <div className="flex justify-between gap-4 py-1"><span className="text-[var(--u-color-text-muted)]">{label}</span><span className="font-medium">{value}</span></div>;
}

function errorTitleKey(recovery: UpdateRecovery): string {
  if (recovery === "check") return "updates.checkFailed";
  if (recovery === "download") return "updates.downloadFailed";
  if (recovery === "signature") return "updates.signatureFailed";
  return "updates.updateFailed";
}

function recoveryMessageKey(recovery: UpdateRecovery): string {
  if (recovery === "installer") return "updates.installerRecovery";
  if (recovery === "download") return "updates.downloadRecovery";
  if (recovery === "signature") return "updates.signatureRecovery";
  return "updates.networkRecovery";
}

export function UpdateDialog() {
  const { t } = useI18n();
  const { meta, state, dialogOpen, setDialogOpen, check, install } = useUpdate();
  if (meta?.distribution === "microsoft-store" || !meta?.updaterEnabled) return null;
  const busy = state.kind === "downloading" || state.kind === "installing";
  const info = state.kind === "available"
      || state.kind === "downloading"
      || state.kind === "installing"
    ? state.info
    : state.kind === "error"
      ? state.info
      : undefined;
  const percent = state.kind === "downloading" && state.total && state.total > 0
    ? Math.min(100, Math.round((state.downloaded / state.total) * 100))
    : null;
  return (
    <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
      <DialogContent title={t("updates.title")}>
        <DialogHeader>
          <DialogTitle>{t("updates.title")}</DialogTitle>
          <DialogXClose label={busy ? t("updates.hide") : t("updates.close")} />
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <div className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-2">
            <Row label={t("updates.currentVersion")} value={meta?.version ?? "—"} />
            <Row label={t("updates.latestVersion")} value={info?.version ?? "—"} />
            <Row label={t("updates.distribution")} value={meta ? t(`updates.${meta.distribution}`) : "—"} />
            <Row label={t("updates.channel")} value={meta ? t(`updates.${meta.channel}`) : "—"} />
          </div>
          {info && (
            <section>
              <h3 className="mb-1 font-semibold">{t("updates.releaseNotes")}</h3>
              <div className="max-h-56 overflow-auto whitespace-pre-wrap rounded-[var(--u-radius-sm)] bg-[var(--u-color-surface-subtle)] p-2 text-xs text-[var(--u-color-text-muted)]">
                {info.body || t("updates.noReleaseNotes")}
              </div>
            </section>
          )}
          {state.kind === "upToDate" && <p className="text-[var(--u-color-success)]">{t("updates.upToDate")}</p>}
          {state.kind === "downloading" && <div><p>{percent === null ? t("updates.downloadingUnknown") : t("updates.downloading", { percent })}</p>{percent !== null && <div className="mt-2 h-1.5 overflow-hidden rounded bg-[var(--u-color-surface-active)]"><div className="h-full bg-[var(--u-color-primary)]" style={{ width: `${percent}%` }} /></div>}</div>}
          {state.kind === "installing" && <p>{t("updates.startingInstaller")}</p>}
          {(info || busy) && <p className="text-xs text-[var(--u-color-text-muted)]">{t("updates.exitNotice")}</p>}
          {busy && <p className="text-xs text-[var(--u-color-text-muted)]">{t("updates.backgroundNotice")}</p>}
          {state.kind === "error" && <div className="space-y-1"><p className="text-[var(--u-color-danger)]">{t(errorTitleKey(state.recovery))}</p><p className="text-xs text-[var(--u-color-text-muted)]">{t("updates.detail", { message: state.message })}</p><p className="text-xs text-[var(--u-color-text-muted)]">{t(recoveryMessageKey(state.recovery))}</p></div>}
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => setDialogOpen(false)} size="sm" type="button" variant="ghost">{busy ? t("updates.hide") : t("updates.close")}</Button>
          {(state.kind === "idle" || state.kind === "checking" || state.kind === "upToDate" || (state.kind === "error" && state.recovery === "check")) && <Button disabled={state.kind === "checking"} onClick={() => void check()} size="sm" type="button">{state.kind === "error" ? t("updates.retryCheck") : state.kind === "checking" ? t("updates.checking") : t("updates.check")}</Button>}
          {(state.kind === "available" || (state.kind === "error" && state.info)) && <Button onClick={() => void install()} size="sm" type="button">{state.kind === "error" ? t("updates.retryInstall") : t("updates.downloadInstall")}</Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function UpdateOverlays() {
  return <UpdateDialog />;
}
