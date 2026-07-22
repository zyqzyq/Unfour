import { Plus, Trash2 } from "lucide-react";
import { Button, Input, cn, useI18n } from "@unfour/ui";
import type { KeyValue } from "@unfour/command-client";

export function KeyValueEditor({
  items,
  onChange,
  showTitle = true,
  title,
}: {
  items: KeyValue[];
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
    "h-[32px] rounded-none border-0 bg-transparent px-0 text-[12px] hover:border-0 focus:border-0 focus:ring-0 focus-visible:outline-none disabled:bg-transparent disabled:text-[var(--u-color-text-soft)]";
  const rows = [...items, { key: "", value: "", enabled: true }];
  const duplicateKeys = findDuplicateKeys(items);

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
        <div className="grid min-h-[28px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <span />
          <span>{t("api.keyValue.key")}</span>
          <span>{t("api.keyValue.value")}</span>
          <span />
        </div>
        {rows.map((item, index) => (
          <div
            className="grid min-h-[34px] grid-cols-[28px_minmax(120px,1fr)_minmax(120px,1fr)_32px] items-center gap-2 border-b border-[var(--u-color-border)] px-2 last:border-b-0 last-of-type:rounded-b-[var(--u-radius-sm)]"
            key={`${title}-${index}`}
          >
            <input
              checked={index === items.length ? false : item.enabled}
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
              type="text"
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
      {duplicateKeys.length > 0 && (
        <div className="rounded-md bg-[var(--u-color-warning-soft)] px-2 py-1 text-[11px] text-[var(--u-color-warning-text)] ring-1 ring-inset ring-[var(--u-badge-warning-ring)]">
          {t("api.keyValue.duplicateKeys", { keys: duplicateKeys.join(", ") })}
        </div>
      )}
    </div>
  );
}

function findDuplicateKeys(items: KeyValue[]): string[] {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const item of items) {
    if (!item.enabled) {
      continue;
    }
    const key = item.key.trim();
    if (!key) {
      continue;
    }
    if (seen.has(key)) {
      duplicates.add(key);
    } else {
      seen.add(key);
    }
  }
  return [...duplicates];
}
