import type { CSSProperties } from "react";
import { cn, useI18n } from "@unfour/ui";

/**
 * History suggestion popup anchored near the terminal cursor. Purely
 * presentational: TerminalPane owns matching, selection state, and the
 * keyboard protocol (Up/Down navigate, Tab or click inserts, Esc dismisses).
 */
export function TerminalCommandSuggestions({
  items,
  onAccept,
  onHoverItem,
  selectedIndex,
  style,
}: {
  items: string[];
  onAccept: (command: string) => void;
  onHoverItem: (index: number) => void;
  selectedIndex: number;
  style?: CSSProperties;
}) {
  const { t } = useI18n();
  return (
    <div
      aria-label={t("ssh.suggest.listLabel")}
      className={cn(
        "absolute z-20 flex w-max min-w-[240px] max-w-[480px] flex-col overflow-hidden",
        "rounded-[var(--u-radius-sm)] border border-[var(--u-color-border-strong)]",
        "bg-[var(--u-color-surface)] shadow-md",
      )}
      role="listbox"
      style={style}
    >
      <div className="max-h-[168px] overflow-y-auto py-0.5">
        {items.map((command, index) => (
          <button
            aria-selected={index === selectedIndex}
            className={cn(
              "flex h-[26px] w-full items-center px-2 text-left",
              "font-mono text-[13px] text-[var(--u-color-text-muted)]",
              index === selectedIndex &&
                "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]",
            )}
            key={`${command}-${index}`}
            onMouseDown={(event) => {
              // Keep focus on the terminal so the inserted text stays editable.
              event.preventDefault();
              onAccept(command);
            }}
            onMouseEnter={() => onHoverItem(index)}
            ref={
              index === selectedIndex
                ? (element) => element?.scrollIntoView?.({ block: "nearest" })
                : undefined
            }
            role="option"
            type="button"
          >
            <span className="truncate">{command}</span>
          </button>
        ))}
      </div>
      <div
        className={cn(
          "border-t border-[var(--u-color-border)] px-2 py-0.5",
          "text-[11px] text-[var(--u-color-text-soft)]",
        )}
      >
        {t("ssh.suggest.hint")}
      </div>
    </div>
  );
}
