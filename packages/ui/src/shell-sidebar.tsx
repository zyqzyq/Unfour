import * as React from "react";
import { clampResizablePaneSize } from "./shell-utils";
import { cn } from "./utils";

export function Sidebar({
  children,
  className,
  collapsed,
  contentClassName,
  footer,
  header,
  maxWidth = 320,
  minWidth = 220,
  onWidthChange,
  resizable = false,
  width = 264,
}: {
  children: React.ReactNode;
  className?: string;
  collapsed?: boolean;
  contentClassName?: string;
  footer?: React.ReactNode;
  header?: React.ReactNode;
  maxWidth?: number;
  minWidth?: number;
  onWidthChange?: (width: number) => void;
  resizable?: boolean;
  width?: number;
}) {
  const hostRef = React.useRef<HTMLElement | null>(null);
  const canResize = resizable && !collapsed && Boolean(onWidthChange);

  function startResize(event: React.PointerEvent<HTMLDivElement>) {
    const initialLeft = hostRef.current?.getBoundingClientRect().left;
    if (initialLeft === undefined || !onWidthChange) {
      return;
    }
    const panelLeft: number = initialLeft;
    const resizePane = onWidthChange;

    event.preventDefault();

    function move(moveEvent: PointerEvent) {
      resizePane(
        clampResizablePaneSize(
          moveEvent.clientX - panelLeft,
          minWidth,
          maxWidth,
          width,
        ),
      );
    }

    function stop() {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    }

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop, { once: true });
  }

  return (
    <aside
      className={cn(
        "relative flex min-h-0 shrink-0 flex-col border-r border-[var(--u-color-border)] bg-[var(--u-color-surface)] transition-[width] duration-150",
        collapsed && "w-[52px]",
        className,
      )}
      ref={hostRef}
      style={collapsed ? undefined : { width }}
    >
      {canResize && (
        <div
          aria-label="Resize sidebar"
          aria-orientation="vertical"
          className="absolute inset-y-0 right-0 z-10 w-1 cursor-col-resize hover:bg-[var(--u-color-focus)]"
          onPointerDown={startResize}
          role="separator"
        />
      )}
      {header}
      <div className={cn("min-h-0 flex-1 overflow-y-auto p-2", contentClassName)}>
        {children}
      </div>
      {footer}
    </aside>
  );
}

export function SidebarHeader({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex h-[var(--u-size-section-toolbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-2",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function SidebarSection({
  children,
  className,
  title,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
}) {
  return (
    <section className={cn("space-y-1", className)}>
      {title && (
        <div className="flex h-7 items-center px-1 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {title}
        </div>
      )}
      {children}
    </section>
  );
}

export function SidebarRow({
  active,
  children,
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean;
}) {
  return (
    <button
      className={cn(
        "flex h-[var(--u-size-sidebar-row)] w-full items-center gap-2 rounded-[var(--u-radius-sm)] px-2 text-left text-[12px] text-[var(--u-color-text-muted)] transition-colors duration-150 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_28%,transparent)]",
        active &&
          "bg-[var(--u-color-primary-soft)] font-semibold text-[var(--u-color-primary)]",
        className,
      )}
      type="button"
      {...props}
    >
      {children}
    </button>
  );
}
