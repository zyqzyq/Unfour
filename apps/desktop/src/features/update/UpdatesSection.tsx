import { Button, useI18n, type TFunction } from "@unfour/ui";
import { useUpdate } from "./useUpdate";
import type { UpdateState } from "./updateTypes";

export function UpdatesSection() {
  const { t } = useI18n();
  const { meta, state, check, openDialog } = useUpdate();
  const managedByStore =
    meta?.distribution === "microsoft-store" || state.kind === "managedByStore";
  const available =
    state.kind === "available"
    || state.kind === "downloading"
    || state.kind === "installing"
    || (state.kind === "error" && Boolean(state.info));
  const status = updateStatusLabel(state, t);
  return (
    <section className="flex flex-col gap-3 border-t border-[var(--u-color-border)] pt-4">
      <div>
        <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
          {t("updates.title")}
        </h3>
        <p className="mt-1 text-[12px] leading-5 text-[var(--u-color-text-muted)]">
          {t("updates.description")}
        </p>
      </div>
      {managedByStore ? (
        <p className="text-sm text-[var(--u-color-text-muted)]">
          {t("updates.managedByMicrosoftStore")}
        </p>
      ) : (
        <>
          <div className="flex items-center justify-between gap-3 border-b border-[var(--u-color-border)] pb-3">
            <span className="text-[12px] text-[var(--u-color-text-muted)]">
              {t("updates.lastStatus")}
            </span>
            <span className="text-right text-[12px] font-medium text-[var(--u-color-text)]">
              {status}
            </span>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={
                state.kind === "checking"
                || state.kind === "downloading"
                || state.kind === "installing"
              }
              onClick={() => void check()}
              size="sm"
              type="button"
            >
              {state.kind === "checking" ? t("updates.checking") : t("updates.check")}
            </Button>
            {available && (
              <Button onClick={openDialog} size="sm" type="button" variant="ghost">
                {t("updates.indicator")}
              </Button>
            )}
          </div>
        </>
      )}
    </section>
  );
}

function updateStatusLabel(state: UpdateState, t: TFunction) {
  switch (state.kind) {
    case "available":
      return t("updates.available", { version: state.info.version });
    case "checking":
      return t("updates.checking");
    case "upToDate":
      return t("updates.upToDate");
    case "downloading":
      return state.total && state.total > 0
        ? t("updates.downloading", {
            percent: Math.min(100, Math.round((state.downloaded / state.total) * 100)),
          })
        : t("updates.downloadingUnknown");
    case "installing":
      return t("updates.startingInstaller");
    case "error":
      return state.info ? t("updates.updateFailed") : t("updates.checkFailed");
    default:
      return t("updates.notChecked");
  }
}
