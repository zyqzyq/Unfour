import * as React from "react";
import { cn } from "./utils";

export type SegmentedControlOption<T extends string> = {
  icon?: React.ReactNode;
  label: React.ReactNode;
  value: T;
};

/**
 * Compact segmented (toggle group) control. Use for small, mutually-exclusive
 * choices where a dropdown would be heavier than warranted (e.g. auth kind).
 */
export function SegmentedControl<T extends string>({
  className,
  onChange,
  options,
  value,
}: {
  className?: string;
  onChange: (value: T) => void;
  options: SegmentedControlOption<T>[];
  value: T;
}) {
  return (
    <div
      className={cn(
        "flex gap-1 overflow-hidden rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-bg)] p-1",
        className,
      )}
      role="radiogroup"
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            aria-checked={selected}
            className={cn(
              "flex h-[var(--u-size-input)] flex-1 items-center justify-center gap-1.5 text-[12px] font-semibold rounded-[var(--u-radius-sm)] transition-colors",
              selected
                ? "bg-[var(--u-color-surface)] text-[var(--u-color-primary)] shadow-[var(--u-shadow-sm)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
            )}
            key={option.value}
            onClick={() => !selected && onChange(option.value)}
            role="radio"
            type="button"
          >
            {option.icon}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
