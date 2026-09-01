import { useEffect, useRef } from "react";
import { Button, IconButton, useI18n } from "@unfour/ui";
import { X } from "lucide-react";
import { openTelemetryPrivacy } from "./privacyLink";
import { useTelemetry } from "./useTelemetry";

export function TelemetryNotice() {
  const { t } = useI18n();
  const {
    dismissNotice,
    markNoticeShown,
    noticeVisible,
    preferenceError,
    setEnabled,
    updating,
  } = useTelemetry();
  const noticeDisplayMarkedRef = useRef(false);

  useEffect(() => {
    if (!noticeVisible || noticeDisplayMarkedRef.current) return;
    noticeDisplayMarkedRef.current = true;
    void markNoticeShown();
  }, [markNoticeShown, noticeVisible]);

  if (!noticeVisible) return null;

  return (
    <aside
      aria-labelledby="telemetry-notice-title"
      className="fixed bottom-8 right-4 z-40 w-[min(460px,calc(100vw-32px))] rounded-[var(--u-radius-md)] border border-[var(--u-color-border-strong)] bg-[var(--u-color-surface)] p-3 text-[var(--u-color-text)]"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-[14px] font-semibold" id="telemetry-notice-title">
            {t("telemetry.notice.title")}
          </h2>
          <p className="mt-1 text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {t("telemetry.notice.description")}
          </p>
          <p className="mt-1 text-[12px] leading-5 text-[var(--u-color-text-muted)]">
            {t("telemetry.notice.exclusions")}
          </p>
        </div>
        <IconButton label={t("telemetry.notice.dismiss")} onClick={dismissNotice} size="compact">
          <X aria-hidden="true" size={15} />
        </IconButton>
      </div>
      {preferenceError && (
        <p className="mt-2 text-[12px] text-[var(--u-color-danger)]">
          {t("telemetry.preferenceError")}
        </p>
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        <Button
          onClick={() => void openTelemetryPrivacy().catch(() => undefined)}
          size="sm"
          type="button"
          variant="ghost"
        >
          {t("telemetry.learnMore")}
        </Button>
        <Button
          disabled={updating}
          onClick={() => void setEnabled(false)}
          size="sm"
          type="button"
          variant="outline"
        >
          {t("telemetry.turnOff")}
        </Button>
      </div>
    </aside>
  );
}
