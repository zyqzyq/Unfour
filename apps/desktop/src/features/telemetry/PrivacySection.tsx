import { Button, LoadingState, useI18n } from "@unfour/ui";
import { openTelemetryPrivacy } from "./privacyLink";
import { useTelemetry } from "./useTelemetry";

export function PrivacySection() {
  const { t } = useI18n();
  const { preferenceError, preferences, setEnabled, updating } = useTelemetry();

  if (!preferences) {
    return <LoadingState>{t("telemetry.settings.loading")}</LoadingState>;
  }

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">
          {t("telemetry.settings.title")}
        </h2>
        <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {t("telemetry.settings.description")}
        </p>
      </div>
      <section className="border-t border-[var(--u-color-border)] pt-3">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <label
              className="text-[12px] font-semibold text-[var(--u-color-text)]"
              htmlFor="telemetry-enabled"
            >
              {t("telemetry.settings.enabledLabel")}
            </label>
            <p className="mt-1 max-w-[560px] text-[12px] leading-5 text-[var(--u-color-text-muted)]">
              {t("telemetry.settings.enabledDescription")}
            </p>
          </div>
          <input
            aria-label={t("telemetry.settings.enabledLabel")}
            checked={preferences.enabled}
            className="mt-0.5 size-4 shrink-0 cursor-pointer accent-[var(--u-color-primary)] disabled:cursor-not-allowed"
            disabled={updating}
            id="telemetry-enabled"
            onChange={(event) => void setEnabled(event.target.checked)}
            role="switch"
            type="checkbox"
          />
        </div>
        {!preferences.networkEnabled && (
          <p className="mt-2 text-[12px] text-[var(--u-color-text-muted)]">
            {t("telemetry.settings.testBuildDisabled")}
          </p>
        )}
        {preferenceError && (
          <p className="mt-2 text-[12px] text-[var(--u-color-danger)]">
            {t("telemetry.preferenceError")}
          </p>
        )}
      </section>
      <div>
        <Button
          onClick={() => void openTelemetryPrivacy().catch(() => undefined)}
          size="sm"
          type="button"
          variant="ghost"
        >
          {t("telemetry.learnMore")}
        </Button>
      </div>
    </div>
  );
}

export function PrivacySectionLabel() {
  const { t } = useI18n();
  return <>{t("telemetry.settings.sectionLabel")}</>;
}
