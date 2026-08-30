import { Button, useI18n } from "@unfour/ui";
import { useUpdate } from "./useUpdate";

function Row({ label, value }: { label: string; value: string }) {
  return <div className="flex justify-between gap-4 py-1 text-sm"><span className="text-[var(--u-color-text-muted)]">{label}</span><span className="font-medium">{value}</span></div>;
}

export function UpdatesSection() {
  const { t } = useI18n();
  const { meta, state, check, openDialog } = useUpdate();
  const managedByStore = meta?.distribution === "microsoft-store" || state.kind === "managedByStore";
  const available = state.kind === "available"
    || state.kind === "downloading"
    || state.kind === "installing"
    || (state.kind === "error" && state.info);
  const status = state.kind === "available"
    ? t("updates.available", { version: state.info.version })
    : state.kind === "checking"
      ? t("updates.checking")
      : state.kind === "upToDate"
        ? t("updates.upToDate")
        : state.kind === "error"
          ? t("updates.checkFailed")
          : t("updates.notChecked");
  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3 py-2">
        <Row label={t("updates.currentVersion")} value={meta?.version ?? "—"} />
        <Row label={t("updates.distribution")} value={meta ? t(`updates.${meta.distribution}`) : "—"} />
        <Row label={t("updates.channel")} value={meta ? t(`updates.${meta.channel}`) : "—"} />
        {!managedByStore && <Row label={t("updates.lastStatus")} value={status} />}
      </div>
      {managedByStore ? (
        <p className="text-sm text-[var(--u-color-text-muted)]">{t("updates.managedByMicrosoftStore")}</p>
      ) : (
        <div className="flex gap-2">
          <Button disabled={state.kind === "checking" || state.kind === "downloading" || state.kind === "installing"} onClick={() => void check()} size="sm" type="button">
            {state.kind === "checking" ? t("updates.checking") : t("updates.check")}
          </Button>
          {available && <Button onClick={openDialog} size="sm" type="button" variant="ghost">{t("updates.indicator")}</Button>}
        </div>
      )}
    </div>
  );
}

export function UpdatesSectionLabel() {
  const { t } = useI18n();
  return <>{t("updates.title")}</>;
}
