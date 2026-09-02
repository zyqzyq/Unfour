import type { ReactNode } from "react";
import {
  Select,
  getLocaleLabel,
  useI18n,
  useTheme,
  type Locale,
  type ThemeMode,
} from "@unfour/ui";
import { SettingsGroup, SettingsRow, SettingsSectionHeading } from "./SettingsPrimitives";

export function SettingsGeneral({ children }: { children?: ReactNode }) {
  const { locale, locales, setLocale, t } = useI18n();
  const { setThemeMode, themeMode } = useTheme();

  return (
    <div className="space-y-5">
      <SettingsSectionHeading
        description={t("app.settings.general.description")}
        title={t("app.settings.general.title")}
      />
      <SettingsGroup title={t("app.settings.general.appearance")}>
        <SettingsRow
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
        <SettingsRow
          control={
            <Select
              aria-label={t("app.settings.general.themeLabel")}
              onChange={(event) => setThemeMode(event.target.value as ThemeMode)}
              options={[
                { label: t("app.theme.system"), value: "system" },
                { label: t("app.theme.light"), value: "light" },
                { label: t("app.theme.dark"), value: "dark" },
              ]}
              value={themeMode}
            />
          }
          description={t("app.settings.general.themeDescription")}
          label={t("app.settings.general.themeLabel")}
        />
      </SettingsGroup>
      {children}
    </div>
  );
}
