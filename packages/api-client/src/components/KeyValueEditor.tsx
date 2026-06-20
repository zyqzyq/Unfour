import { Plus, Trash2 } from "lucide-react";
import { Button, Input, cn, useI18n } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";
import { duplicateEnvironmentKeys, isSensitiveKey } from "../request-utils";

export function KeyValueEditor({
  items,
  maskSensitiveValues = false,
  onChange,
  showTitle = true,
  title,
}: {
  items: KeyValue[];
  maskSensitiveValues?: boolean;
  onChange: (items: KeyValue[]) => void;
  showTitle?: boolean;
  title: string;
}) {
  const { t } = useI18n();

  function update(index: number, patch: Partial<KeyValue>) {
    if (index === items.length) {
      onChange([...items, { enabled: true, key: "", value: "", ...patch }]);
      return;
    }
    onChange(items.map((item, itemIndex) => (itemIndex === index ? { ...item, ...patch } : item)));
  }

  function remove(index: number) {
    onChange(items.filter((_, itemIndex) => itemIndex !== index));
  }

  const cellInputClass =
    "h-[32px] rounded-none border-0 bg-transparent px-0 text-[12px] hover:border-0 focus:border-0 focus:ring-0 disabled:bg-transparent disabled:text-[var(--u-color-text-soft)]";
  const rows = [...items, { key: "", value: "", enabled: true }];

  return (
    <div className="space-y-1.5">
      <div className={cn("flex items-center", showTitle ? "justify-between" : "justify-end")}>
        {showTitle && (
          <span className="text-xs font-semibold uppercase text-[var(--u-color-text-muted)]">
            {title}
          </span>
        )}
        <Button
          onClick={() => onChange([...items, { key: "", value: "", enabled: true }])}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus size={13} />
          {t("api.keyValue.add")}
        </Button>
      </div>
      <div className="overflow-hidden rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
        <div className="grid min-h-[28px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <span />
          <span>{t("api.keyValue.key")}</span>
          <span>{t("api.keyValue.value")}</span>
          <span />
        </div>
        {rows.map((item, index) => (
          <div
            className="grid min-h-[34px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center gap-2 border-b border-[var(--u-color-border)] px-2 last:border-b-0"
            key={`${title}-${index}`}
          >
            <input
              checked={item.enabled}
              className="h-4 w-4"
              onChange={(event) => update(index, { enabled: event.target.checked })}
              type="checkbox"
            />
            <Input
              className={cellInputClass}
              onChange={(event) => update(index, { key: event.target.value })}
              placeholder={t("api.keyValue.key")}
              value={item.key}
            />
            <Input
              className={cellInputClass}
              onChange={(event) => update(index, { value: event.target.value })}
              placeholder={t("api.keyValue.value")}
              type={maskSensitiveValues && isSensitiveKey(item.key) ? "password" : "text"}
              value={item.value}
            />
            <button
              aria-label={t("api.keyValue.deleteRow", { title })}
              className="grid h-7 w-7 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)] disabled:pointer-events-none disabled:opacity-0"
              disabled={index === items.length}
              onClick={() => remove(index)}
              type="button"
            >
              <Trash2 size={13} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

export function EnvironmentHints({ variables }: { variables: KeyValue[] }) {
  const { t } = useI18n();
  const duplicateKeys = duplicateEnvironmentKeys(variables);
  const sensitiveKeys = variables
    .filter((item) => item.enabled && isSensitiveKey(item.key) && item.value.trim())
    .map((item) => item.key.trim());

  if (!duplicateKeys.length && !sensitiveKeys.length) {
    return null;
  }

  return (
    <div className="mt-2 space-y-1 text-xs">
      {duplicateKeys.length > 0 && (
        <div className="rounded-md bg-[var(--u-color-warning-soft)] px-2 py-1 text-[var(--u-color-warning-text)] ring-1 ring-inset ring-[var(--u-badge-warning-ring)]">
          {t("api.keyValue.duplicateVariables", { keys: duplicateKeys.join(", ") })}
        </div>
      )}
      {sensitiveKeys.length > 0 && (
        <div className="rounded-md bg-[var(--u-badge-neutral-bg)] px-2 py-1 text-[var(--u-color-text-muted)] ring-1 ring-inset ring-[var(--u-badge-neutral-ring)]">
          {t("api.keyValue.sensitiveMasked", { keys: sensitiveKeys.join(", ") })}
        </div>
      )}
    </div>
  );
}
