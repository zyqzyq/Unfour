import * as React from "react";
import { cn } from "./utils";
import { clampResizablePaneSize } from "./shell-utils";
import { ResizableSplitPane } from "./adapters/resizable-panels";
import { usePlatform } from "./platform";

export function AppShellFrame({
  activityBar,
  bottomPanel,
  children,
  className,
  globalToolbar,
  rightInspector,
  sidebar,
  statusBar,
}: {
  activityBar?: React.ReactNode;
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
        {activityBar}
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

export function ActivityBar({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <aside
      className={cn(
        "flex w-[48px] shrink-0 flex-col items-center border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] py-2",
        className,
      )}
    >
      {children}
    </aside>
  );
}

export function GlobalToolbar({
  center,
  className,
  left,
  right,
}: {
  center?: React.ReactNode;
  className?: string;
  left?: React.ReactNode;
  right?: React.ReactNode;
}) {
  const isMac = usePlatform() === "macos";
  return (
    <header
      className={cn(
        "flex h-[var(--u-size-global-toolbar)] shrink-0 items-center border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text)]",
        className,
      )}
    >
      <div
        className="flex h-full shrink-0 items-center gap-1 px-2"
        style={isMac ? { paddingLeft: 72 } : undefined}
      >
        {left}
      </div>
      <div
        className="flex h-full min-w-0 flex-1 items-center justify-center px-3"
        data-tauri-drag-region=""
      >
        {center}
      </div>
      <div className="flex h-full shrink-0 items-center gap-1 px-2">{right}</div>
    </header>
  );
}

export {
  Sidebar,
  SidebarHeader,
  SidebarRow,
  SidebarSection,
} from "./shell-sidebar";

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
    <main className={cn("flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-bg)]", className)}>
      {tabBar}
      <section className="min-h-0 flex-1 overflow-hidden p-2">{children}</section>
    </main>
  );
}

export function RightInspector({
  children,
  className,
  collapsed,
  maxWidth = 420,
  minWidth = 260,
  onWidthChange,
  resizable = false,
  width = 300,
}: {
  children?: React.ReactNode;
  className?: string;
  collapsed?: boolean;
  maxWidth?: number;
  minWidth?: number;
  onWidthChange?: (width: number) => void;
  resizable?: boolean;
  width?: number;
}) {
  const hostRef = React.useRef<HTMLElement | null>(null);

  if (collapsed) {
    return null;
  }

  function startResize(event: React.PointerEvent<HTMLDivElement>) {
    const initialRight = hostRef.current?.getBoundingClientRect().right;
    if (initialRight === undefined || !onWidthChange) {
      return;
    }
    const panelRight: number = initialRight;
    const resizePane = onWidthChange;

    event.preventDefault();

    function move(moveEvent: PointerEvent) {
      resizePane(
        clampResizablePaneSize(
          panelRight - moveEvent.clientX,
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
        "relative min-h-0 shrink-0 border-l border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]",
        className,
      )}
      ref={hostRef}
      style={{ width }}
    >
      {resizable && onWidthChange && (
        <div
          aria-label="Resize inspector"
          aria-orientation="vertical"
          className="absolute inset-y-0 left-0 z-10 w-1 cursor-col-resize hover:bg-[var(--u-color-focus)]"
          onPointerDown={startResize}
          role="separator"
        />
      )}
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
        "u-statusbar flex h-[var(--u-size-statusbar)] shrink-0 items-center justify-between gap-3 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2 text-[11px] text-[var(--u-color-text-muted)]",
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
  onRatioChange,
  orientation = "horizontal",
  resizable = false,
}: {
  children: React.ReactNode;
  className?: string;
  defaultRatio?: number;
  minPaneSize?: number;
  /** Optional callback receiving the first pane's percentage (0-100) on resize. */
  onRatioChange?: (ratio: number) => void;
  orientation?: "horizontal" | "vertical";
  resizable?: boolean;
}) {
  const panes = React.Children.toArray(children);

  if (!resizable || panes.length !== 2) {
    return <div className={cn("flex min-h-0 min-w-0 flex-1", className)}>{children}</div>;
  }

  return (
    <ResizableSplitPane
      className={className}
      defaultRatio={defaultRatio}
      minPaneSize={minPaneSize}
      onRatioChange={onRatioChange}
      orientation={orientation}
    >
      {[panes[0], panes[1]]}
    </ResizableSplitPane>
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

