import * as React from "react";
import { cn } from "./utils";

type IconButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  tooltip?: string;
  size?: "default" | "compact";
};

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ children, className, label, size = "default", tooltip, type = "button", ...props }, ref) => (
    <button
      aria-label={label}
      className={cn(
        "group relative inline-flex shrink-0 select-none items-center justify-center rounded-[var(--u-radius-sm)] border border-transparent text-[var(--u-color-text-muted)] transition-colors duration-150 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]",
        size === "compact"
          ? "h-[var(--u-size-button-compact)] w-[var(--u-size-button-compact)]"
          : "h-[var(--u-size-button)] w-[var(--u-size-button)]",
        className,
      )}
      ref={ref}
      title={tooltip ?? label}
      type={type}
      {...props}
    >
      {children}
      <span className="pointer-events-none absolute left-1/2 top-full z-50 mt-1 hidden -translate-x-1/2 whitespace-nowrap rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-text)] px-2 py-1 text-[11px] font-medium text-[var(--u-color-surface)] shadow-sm group-hover:block group-focus-visible:block">
        {tooltip ?? label}
      </span>
    </button>
  ),
);

IconButton.displayName = "IconButton";
