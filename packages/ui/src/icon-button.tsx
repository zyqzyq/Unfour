import * as React from "react";
import { cn } from "./utils";

type IconButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  tooltip?: string;
  /** Align edge actions' tooltips inside their containing panel. */
  tooltipAlign?: "start" | "center" | "end";
  /** Hide the built-in hover/focus tooltip. Use inside scroll containers where
   *  the absolutely-positioned tooltip would otherwise create a phantom
   *  scrollbar (e.g. row action menus in a sidebar tree). The native `title`
   *  and `aria-label` are kept for accessibility. */
  disableTooltip?: boolean;
  size?: "default" | "compact";
};

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ children, className, disableTooltip = false, label, size = "default", tooltip, tooltipAlign = "center", type = "button", ...props }, ref) => (
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
      {!disableTooltip && (
        <span className={cn(
          "pointer-events-none absolute top-full z-50 mt-1 hidden whitespace-nowrap rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-text)] px-2 py-1 text-[11px] font-medium text-[var(--u-color-surface)] shadow-sm group-hover:block group-focus-visible:block",
          { start: "left-0", center: "left-1/2 -translate-x-1/2", end: "right-0" }[tooltipAlign],
        )}>
          {tooltip ?? label}
        </span>
      )}
    </button>
  ),
);

IconButton.displayName = "IconButton";
