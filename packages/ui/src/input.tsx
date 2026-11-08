import * as React from "react";
import { cn } from "./utils";

export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, ...props }, ref) => (
  <input
    className={cn(
      "h-[var(--u-size-input)] w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-canvas)] px-2.5 text-[13px] text-[var(--u-color-text)] outline-none transition-colors duration-150 placeholder:text-[var(--u-color-text-soft)] hover:border-[var(--u-color-border-strong)] focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)] disabled:cursor-not-allowed disabled:bg-[var(--u-color-surface-muted)] disabled:text-[var(--u-color-text-soft)]",
      className,
    )}
    ref={ref}
    {...props}
  />
));

Input.displayName = "Input";
