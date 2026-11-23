import { Select, useI18n } from "@unfour/ui";

const connectionTypes = ["postgres", "opengauss", "mysql", "sqlite", "redis"] as const;
export type DatabaseConnectionType = typeof connectionTypes[number];

export function DatabaseConnectionTypeSelect({ value, onChange, disabled, autoFocus }: {
  value: DatabaseConnectionType;
  onChange?: (type: DatabaseConnectionType) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}) {
  const { t } = useI18n();
  const options = (onChange ? connectionTypes : [value]).map((type) => ({
    value: type,
    label: t(`database.driver.${type}`),
  }));

  return (
    <label className="block space-y-1.5">
      <span className="text-[12px] font-medium text-[var(--u-color-text-muted)]">{t("database.fields.driver")}</span>
      <Select autoFocus={autoFocus} disabled={disabled || !onChange} value={value} options={options} onChange={(event) => {
        const type = connectionTypes.find((option) => option === event.target.value);
        if (type && !disabled) onChange?.(type);
      }} />
    </label>
  );
}
