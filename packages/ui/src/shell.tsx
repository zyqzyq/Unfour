import * as React from "react";
import { cn } from "./utils";

export type ShellTab = {
  id: string;
  title: string;
  meta?: string;
};

export function AppShellFrame({
  bottomPanel,
  children,
  className,
  globalToolbar,
  rightInspector,
  sidebar,
  statusBar,
}: {
  bottomPanel?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  globalToolbar: React.ReactNode;
  rightInspector?: React.ReactNode;
  sidebar: React.ReactNode;
  statusBar?: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "app-shell flex h-screen min-h-[680px] flex-col bg-[var(--u-color-bg)] text-[13px] leading-[var(--u-line-height-ui)] text-[var(--u-color-text)]",
        className,
      )}
    >
      {globalToolbar}
      <div className="flex min-h-0 flex-1">
        {sidebar}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex min-h-0 flex-1">
            <div className="flex min-w-0 flex-1 flex-col">{children}</div>
            {rightInspector}
          </div>
          {bottomPanel}
        </div>
      </div>
      {statusBar}
    </div>
  );
}

export function GlobalToolbar({
  center,
  className,
  left,
  onDragRegionMouseDown,
  right,
}: {
  center?: React.ReactNode;
  className?: string;
  left?: React.ReactNode;
  onDragRegionMouseDown?: React.MouseEventHandler<HTMLDivElement>;
  right?: React.ReactNode;
}) {
  return (
    <header
      className={cn(
        "flex h-[var(--u-size-global-toolbar)] shrink-0 items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]",
        className,
      )}
    >
      <div className="flex h-full shrink-0 items-center gap-1 px-2">{left}</div>
      <div
        className="flex h-full min-w-0 flex-1 items-center justify-center px-3"
        onMouseDown={onDragRegionMouseDown}
      >
        {center}
      </div>
      <div className="flex h-full shrink-0 items-center gap-1 px-2">{right}</div>
    </header>
  );
}

export function Sidebar({
  children,
  className,
  collapsed,
  footer,
  header,
}: {
  children: React.ReactNode;
  className?: string;
  collapsed?: boolean;
  footer?: React.ReactNode;
  header?: React.ReactNode;
}) {
  return (
    <aside
      className={cn(
        "flex min-h-0 shrink-0 flex-col border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] transition-[width] duration-150",
        collapsed ? "w-[52px]" : "w-[264px] max-[900px]:w-[220px]",
        className,
      )}
    >
      {header}
      <div className="min-h-0 flex-1 overflow-y-auto p-2">{children}</div>
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
          "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)] ring-1 ring-inset ring-[var(--u-color-border)]",
        className,
      )}
      type="button"
      {...props}
    >
      {children}
    </button>
  );
}

export function TabBar({
  activeTabId,
  className,
  onSelectTab,
  tabs,
}: {
  activeTabId: string;
  className?: string;
  onSelectTab: (tabId: string) => void;
  tabs: ShellTab[];
}) {
  return (
    <div
      className={cn(
        "flex h-[var(--u-size-tabbar)] shrink-0 items-end border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2",
        className,
      )}
    >
      {tabs.map((tab) => {
        const active = tab.id === activeTabId;
        return (
          <button
            className={cn(
              "flex h-[30px] min-w-[116px] max-w-[220px] items-center gap-2 rounded-t-[var(--u-radius-sm)] border border-transparent px-3 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors duration-150",
              active
                ? "border-[var(--u-color-border)] border-b-[var(--u-color-surface)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
            )}
            key={tab.id}
            onClick={() => onSelectTab(tab.id)}
            type="button"
          >
            <span className="min-w-0 flex-1 truncate text-left">{tab.title}</span>
            {tab.meta && (
              <span className="shrink-0 text-[11px] text-[var(--u-color-text-soft)]">
                {tab.meta}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

export function MainWorkspace({
  children,
  className,
  tabBar,
}: {
  children: React.ReactNode;
  className?: string;
  tabBar: React.ReactNode;
}) {
  return (
    <main className={cn("flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-surface)]", className)}>
      {tabBar}
      <section className="min-h-0 flex-1 overflow-hidden p-2">{children}</section>
    </main>
  );
}

export function RightInspector({
  children,
  className,
  collapsed,
  width = 300,
}: {
  children?: React.ReactNode;
  className?: string;
  collapsed?: boolean;
  width?: number;
}) {
  if (collapsed) {
    return null;
  }

  return (
    <aside
      className={cn(
        "min-h-0 shrink-0 border-l border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]",
        className,
      )}
      style={{ width }}
    >
      {children}
    </aside>
  );
}

export function BottomPanel({
  children,
  className,
  collapsed,
  height = 220,
  maxHeight = 480,
  minHeight = 120,
  onHeightChange,
  resizable = false,
}: {
  children?: React.ReactNode;
  className?: string;
  collapsed?: boolean;
  height?: number;
  maxHeight?: number;
  minHeight?: number;
  onHeightChange?: (height: number) => void;
  resizable?: boolean;
}) {
  const hostRef = React.useRef<HTMLElement | null>(null);

  if (collapsed) {
    return null;
  }

  function startResize(event: React.PointerEvent<HTMLDivElement>) {
    const initialBottom = hostRef.current?.getBoundingClientRect().bottom;
    if (initialBottom === undefined || !onHeightChange) {
      return;
    }
    const panelBottom: number = initialBottom;

    event.preventDefault();

    function move(moveEvent: PointerEvent) {
      onHeightChange?.(
        Math.min(Math.max(panelBottom - moveEvent.clientY, minHeight), maxHeight),
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
    <section
      className={cn(
        "relative shrink-0 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]",
        className,
      )}
      ref={hostRef}
      style={{ height }}
    >
      {resizable && onHeightChange && (
        <div
          aria-label="Resize bottom panel"
          aria-orientation="vertical"
          className="absolute inset-x-0 top-0 z-10 h-1 cursor-row-resize hover:bg-[var(--u-color-focus)]"
          onPointerDown={startResize}
          role="separator"
        />
      )}
      {children}
    </section>
  );
}

export function StatusBar({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <footer
      className={cn(
        "flex h-[var(--u-size-statusbar)] shrink-0 items-center justify-between gap-3 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] text-[var(--u-color-text-muted)]",
        className,
      )}
    >
      {children}
    </footer>
  );
}

export function SplitPane({
  children,
  className,
  defaultRatio = 50,
  minPaneSize = 160,
  orientation = "horizontal",
  resizable = false,
}: {
  children: React.ReactNode;
  className?: string;
  defaultRatio?: number;
  minPaneSize?: number;
  orientation?: "horizontal" | "vertical";
  resizable?: boolean;
}) {
  const hostRef = React.useRef<HTMLDivElement | null>(null);
  const [ratio, setRatio] = React.useState(defaultRatio);
  const panes = React.Children.toArray(children);

  if (!resizable || panes.length !== 2) {
    return <div className={cn("flex min-h-0 min-w-0 flex-1", className)}>{children}</div>;
  }

  function resize(clientX: number, clientY: number) {
    const bounds = hostRef.current?.getBoundingClientRect();
    if (!bounds) {
      return;
    }

    const total = orientation === "horizontal" ? bounds.width : bounds.height;
    const offset =
      orientation === "horizontal" ? clientX - bounds.left : clientY - bounds.top;
    const minimumRatio = (minPaneSize / Math.max(total, 1)) * 100;
    const nextRatio = Math.min(
      Math.max((offset / Math.max(total, 1)) * 100, minimumRatio),
      100 - minimumRatio,
    );
    setRatio(nextRatio);
  }

  function startResize(event: React.PointerEvent<HTMLDivElement>) {
    event.preventDefault();

    function move(moveEvent: PointerEvent) {
      resize(moveEvent.clientX, moveEvent.clientY);
    }

    function stop() {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    }

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop, { once: true });
  }

  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 flex-1",
        orientation === "vertical" && "flex-col",
        className,
      )}
      ref={hostRef}
    >
      <div
        className="flex min-h-0 min-w-0 shrink-0"
        style={{
          flexBasis: `${ratio}%`,
        }}
      >
        {panes[0]}
      </div>
      <div
        aria-label={`Resize ${orientation === "horizontal" ? "horizontal" : "vertical"} split`}
        aria-orientation={orientation}
        className={cn(
          "shrink-0 bg-[var(--u-color-border)] hover:bg-[var(--u-color-focus)]",
          orientation === "horizontal"
            ? "w-px cursor-col-resize"
            : "h-px cursor-row-resize",
        )}
        onPointerDown={startResize}
        role="separator"
      />
      <div className="flex min-h-0 min-w-0 flex-1">{panes[1]}</div>
    </div>
  );
}

export function CommandPalette({
  actions,
  onClose,
  open,
}: {
  actions?: React.ReactNode;
  onClose: () => void;
  open: boolean;
}) {
  if (!open) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-[color:color-mix(in_srgb,var(--u-color-text)_24%,transparent)] pt-[14vh]">
      <div className="w-[min(640px,calc(100vw-32px))] overflow-hidden rounded-[var(--u-radius-lg)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] shadow-lg">
        <div className="border-b border-[var(--u-color-border)] p-2">
          <input
            autoFocus
            className="h-[var(--u-size-input)] w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] px-2 text-[13px] text-[var(--u-color-text)] outline-none focus:border-[var(--u-color-focus)]"
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                onClose();
              }
            }}
            placeholder="Search commands"
          />
        </div>
        <div className="max-h-[360px] overflow-y-auto p-1 text-[13px]">{actions}</div>
      </div>
      <button aria-label="Close command palette" className="absolute inset-0 -z-10" onClick={onClose} type="button" />
    </div>
  );
}
