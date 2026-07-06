import type { ReactNode } from "react";
import {
  Select,
  getLocaleLabel,
  useI18n,
  useTheme,
  type Locale,
  type Theme,
} from "@unfour/ui";

export function SettingsGeneral() {
  const { locale, locales, setLocale, t } = useI18n();
  const { setTheme, theme } = useTheme();

  return (
    <div className="space-y-4">
      <SectionHeading
        description={t("app.settings.general.description")}
        title={t("app.settings.general.title")}
      />
      <SettingRow
        control={
          <Select
            aria-label={t("app.settings.general.languageLabel")}
            onChange={(event) => setLocale(event.target.value as Locale)}
            options={locales.map((item) => ({
              label: getLocaleLabel(item),
              value: item,
            }))}
            value={locale}
          />
        }
        description={t("app.settings.general.languageDescription")}
        label={t("app.settings.general.languageLabel")}
      />
      <SettingRow
        control={
          <Select
            aria-label={t("app.settings.general.themeLabel")}
            onChange={(event) => setTheme(event.target.value as Theme)}
            options={[
              { label: t("app.theme.light"), value: "light" },
              { label: t("app.theme.dark"), value: "dark" },
            ]}
            value={theme}
          />
        }
        description={t("app.settings.general.themeDescription")}
        label={t("app.settings.general.themeLabel")}
      />
    </div>
  );
}

function SectionHeading({
  description,
  title,
}: {
  description: string;
  title: string;
}) {
  return (
    <div>
      <h2 className="text-[14px] font-semibold text-[var(--u-color-text)]">{title}</h2>
      <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">{description}</p>
    </div>
  );
}

function SettingRow({
  control,
  description,
  label,
}: {
  control: ReactNode;
  description: string;
  label: string;
}) {
  return (
    <div className="grid grid-cols-[150px_minmax(0,1fr)] gap-3 border-t border-[var(--u-color-border)] pt-3">
      <div>
        <div className="text-[12px] font-semibold text-[var(--u-color-text)]">{label}</div>
        <div className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
          {description}
        </div>
      </div>
      <div className="max-w-[260px]">{control}</div>
    </div>
  );
}
