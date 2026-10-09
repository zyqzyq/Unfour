import { Eye, EyeOff, Plus, Trash2, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button } from "./button";
import { Input } from "./input";
import { useI18n } from "./i18n";
import { cn } from "./utils";

export type VariableTableItem = {
  id?: string | null;
  key: string;
  value: string;
  isSecret: boolean;
  isEnabled: boolean;
  description: string | null;
  sortOrder: number;
};

export function VariableTable<T extends VariableTableItem>({
  items,
  onChange,
  overridingKeys,
  renderDecoration,
  valuePresentation,
  title,
}: {
  items: T[];
  onChange: (items: T[]) => void;
  overridingKeys?: ReadonlySet<string>;
  renderDecoration?: (item: T) => ReactNode;
  valuePresentation?: (item: T) => {
    value: string;
    placeholder?: string;
    canReveal?: boolean;
    clearLabel?: string;
    secretToggleDisabled?: boolean;
  };
  title: string;
}) {
  const { t } = useI18n();
  const [revealedIds, setRevealedIds] = useState<Set<string>>(() => new Set());
  const duplicateKeys = findDuplicateKeys(items);
  const normalizedOverridingKeys = new Set(
    [...(overridingKeys ?? [])].map((key) => key.trim().toLowerCase()).filter(Boolean),
  );
  const cellInputClass =
    "h-[32px] rounded-none border-0 bg-transparent px-0 text-[12px] hover:border-0 focus:border-0 focus:ring-0 focus-visible:outline-none disabled:bg-transparent disabled:text-[var(--u-color-text-soft)]";

  function update(index: number, patch: Partial<T>) {
    onChange(
      items.map((item, itemIndex) =>
        itemIndex === index ? ({ ...item, ...patch } as T) : item,
      ),
    );
  }

  function remove(index: number) {
    onChange(items.filter((_, itemIndex) => itemIndex !== index));
  }

  function add() {
    onChange([
      ...items,
      {
        key: "",
        value: "",
        isSecret: false,
        isEnabled: true,
        description: null,
        sortOrder: items.length,
      } as T,
    ]);
  }

  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase text-[var(--u-color-text-muted)]">
          {title}
        </span>
        <Button onClick={add} size="sm" type="button" variant="ghost">
          <Plus size={13} />
          {t("variables.add")}
        </Button>
      </div>
      <div className="overflow-x-auto rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)]">
        <div
          className={cn(
            "grid min-h-[28px] min-w-[720px] items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]",
            renderDecoration
              ? "grid-cols-[28px_minmax(150px,1fr)_minmax(190px,1.3fr)_70px_minmax(150px,1fr)_auto_32px]"
              : "grid-cols-[28px_minmax(150px,1fr)_minmax(190px,1.3fr)_70px_minmax(150px,1fr)_32px]",
          )}
        >
          <span title={t("variables.enabled")}>{t("variables.enabledShort")}</span>
          <span>{t("variables.key")}</span>
          <span>{t("variables.value")}</span>
          <span>{t("variables.secret")}</span>
          <span>{t("variables.description")}</span>
          {renderDecoration && <span />}
          <span />
        </div>
        {items.length === 0 ? (
          <div className="px-3 py-5 text-center text-[12px] text-[var(--u-color-text-muted)]">
            {t("variables.empty")}
          </div>
        ) : (
          items.map((item, index) => {
            const rowId = item.id ?? `new-${index}`;
            const revealed = revealedIds.has(rowId);
            const presentation = valuePresentation?.(item);
            const normalizedKey = item.key.trim().toLowerCase();
            const overrides = Boolean(
              normalizedKey && normalizedOverridingKeys.has(normalizedKey),
            );
            return (
              <div
                className={cn(
                  "grid min-h-[38px] min-w-[720px] items-center gap-2 border-b border-[var(--u-color-border)] px-2 last:border-b-0",
                  renderDecoration
                    ? "grid-cols-[28px_minmax(150px,1fr)_minmax(190px,1.3fr)_70px_minmax(150px,1fr)_auto_32px]"
                    : "grid-cols-[28px_minmax(150px,1fr)_minmax(190px,1.3fr)_70px_minmax(150px,1fr)_32px]",
                )}
                key={rowId}
              >
                <input
                  aria-label={t("variables.enabled")}
                  checked={item.isEnabled}
                  className="h-4 w-4 cursor-pointer"
                  onChange={(event) => update(index, { isEnabled: event.target.checked } as Partial<T>)}
                  type="checkbox"
                />
                <div className="min-w-0">
                  <Input
                    aria-invalid={
                      duplicateKeys.has(item.key.trim().toLowerCase()) || undefined
                    }
                    className={cellInputClass}
                    onChange={(event) => update(index, { key: event.target.value } as Partial<T>)}
                    placeholder={t("variables.key")}
                    value={item.key}
                  />
                  {overrides && (
                    <div className="-mt-1 truncate pb-1 text-[10px] text-[var(--u-color-warning-text)]">
                      {t("variables.overridesWorkspace")}
                    </div>
                  )}
                </div>
                <div className="flex min-w-0 items-center">
                  <Input
                    className={cn(cellInputClass, "min-w-0 flex-1 pr-7")}
                    onChange={(event) => update(index, { value: event.target.value } as Partial<T>)}
                    placeholder={presentation?.placeholder ?? t("variables.value")}
                    type={item.isSecret && !revealed ? "password" : "text"}
                    value={presentation?.value ?? item.value}
                  />
                  {item.isSecret && presentation?.canReveal !== false && (
                    <button
                      aria-label={revealed ? t("variables.hideSecret") : t("variables.showSecret")}
                      className="-ml-7 grid h-7 w-7 cursor-pointer place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
                      onClick={() =>
                        setRevealedIds((current) => {
                          const next = new Set(current);
                          if (revealed) next.delete(rowId);
                          else next.add(rowId);
                          return next;
                        })
                      }
                      type="button"
                    >
                      {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
                    </button>
                  )}
                  {presentation?.clearLabel && (
                    <Button
                      aria-label={presentation.clearLabel}
                      onClick={() => update(index, { value: "" } as Partial<T>)}
                      size="icon"
                      className="h-7 w-7"
                      type="button"
                      variant="ghost"
                    >
                      <X size={13} />
                    </Button>
                  )}
                </div>
                <label className="flex cursor-pointer items-center gap-1 text-[11px] text-[var(--u-color-text-muted)]">
                  <input
                    checked={item.isSecret}
                    disabled={presentation?.secretToggleDisabled}
                    className="h-4 w-4 cursor-pointer"
                    onChange={(event) => update(index, { isSecret: event.target.checked } as Partial<T>)}
                    type="checkbox"
                  />
                  {t("variables.secret")}
                </label>
                <Input
                  className={cellInputClass}
                  onChange={(event) =>
                    update(index, { description: event.target.value || null } as Partial<T>)
                  }
                  placeholder={t("variables.description")}
                  value={item.description ?? ""}
                />
                {renderDecoration && <div className="shrink-0">{renderDecoration(item)}</div>}
                <button
                  aria-label={t("variables.deleteRow", { key: item.key || String(index + 1) })}
                  className="grid h-7 w-7 cursor-pointer place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)]"
                  onClick={() => remove(index)}
                  type="button"
                >
                  <Trash2 size={13} />
                </button>
              </div>
            );
          })
        )}
      </div>
      {duplicateKeys.size > 0 && (
        <div className="rounded-[var(--u-radius-sm)] bg-[var(--u-color-warning-soft)] px-2 py-1 text-[11px] text-[var(--u-color-warning-text)] ring-1 ring-inset ring-[var(--u-badge-warning-ring)]">
          {t("variables.duplicateKeys", { keys: [...duplicateKeys].join(", ") })}
        </div>
      )}
    </div>
  );
}

function findDuplicateKeys(items: VariableTableItem[]) {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const item of items) {
    const key = item.key.trim();
    if (!key) continue;
    const normalized = key.toLowerCase();
    if (seen.has(normalized)) duplicates.add(normalized);
    else seen.add(normalized);
  }
  return duplicates;
}
