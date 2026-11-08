import * as React from "react";
import { cn } from "./utils";

export type SelectOption = {
  disabled?: boolean;
  label: string;
  value: string;
};

export function Select({
  className,
  options,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement> & {
  options?: SelectOption[];
}) {
  return (
    <select
      className={cn(
        "h-[var(--u-size-input)] w-full min-w-0 truncate rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-canvas)] py-0 pl-2 pr-8 text-[13px] text-[var(--u-color-text)] outline-none transition-colors hover:border-[var(--u-color-border-strong)] focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_20%,transparent)] disabled:cursor-not-allowed disabled:opacity-60",
        className,
      )}
      {...props}
    >
      {options?.map((option) => (
        <option disabled={option.disabled} key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
      {props.children}
    </select>
  );
}
