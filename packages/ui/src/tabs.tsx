import * as React from "react";
import { GripVertical, X } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "./menus";
import { cn } from "./utils";

export type WorkspaceTab = {
  /** Right-click menu content (ContextMenuItem nodes) for this tab. */
  contextMenu?: React.ReactNode;
  /** Whether this tab can be reordered via drag. */
  draggable?: boolean;
  id: string;
  loading?: boolean;
  meta?: React.ReactNode;
  /** Shows an unsaved indicator dot and triggers confirmation on close. */
  modified?: boolean;
  title: string;
};

export type TabsAction =
  | { close: string }
  | { closeOthers: string; keep: string }
  | { closeRight: string; keep: string };

export function Tabs({
  activeId,
  className,
  endControl,
  onAction,
  onClose,
  onReorder,
  onSelect,
  tabs,
}: {
  activeId: string;
  className?: string;
  /** Optional element rendered at the right edge of the tab bar. */
  endControl?: React.ReactNode;
  /** Unified handler for close/close-others/close-right actions and custom actions. */
  onAction?: (action: TabsAction) => void;
  onClose?: (tabId: string) => void;
  onReorder?: (fromIndex: number, toIndex: number) => void;
  onSelect: (tabId: string) => void;
  tabs: WorkspaceTab[];
}) {
  const dragRef = React.useRef<{ id: string; index: number } | null>(null);
  const [dragOverIndex, setDragOverIndex] = React.useState<number | null>(null);
  const [confirmTabId, setConfirmTabId] = React.useState<string | null>(null);
  const [confirmResolve, setConfirmResolve] = React.useState<((confirmed: boolean) => void) | null>(null);

  const handleClose = React.useCallback(
    (tabId: string) => {
      const tab = tabs.find((t) => t.id === tabId);
      if (tab?.modified) {
        setConfirmTabId(tabId);
        setConfirmResolve(() => (confirmed: boolean) => {
          if (confirmed) onClose?.(tabId);
          setConfirmTabId(null);
          setConfirmResolve(null);
        });
      } else {
        onClose?.(tabId);
      }
    },
    [onClose, tabs],
  );

  const handleDragStart = React.useCallback(
    (e: React.DragEvent, tabId: string, index: number) => {
      if (e.dataTransfer) {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/tab-id", tabId);
      }
      dragRef.current = { id: tabId, index };
    },
    [],
  );

  const handleDragOver = React.useCallback(
    (e: React.DragEvent, index: number) => {
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
      setDragOverIndex(index);
    },
    [],
  );

  const handleDrop = React.useCallback(
    (e: React.DragEvent, toIndex: number) => {
      e.preventDefault();
      setDragOverIndex(null);
      const from = dragRef.current;
      if (from && from.index !== toIndex) {
        onReorder?.(from.index, toIndex);
      }
      dragRef.current = null;
    },
    [onReorder],
  );

  const handleDragEnd = React.useCallback(() => {
    dragRef.current = null;
    setDragOverIndex(null);
  }, []);

  const handleCloseOthers = React.useCallback(
    (keepId: string) => {
      tabs.forEach((t) => {
        if (t.id !== keepId) onClose?.(t.id);
      });
    },
    [onClose, tabs],
  );

  const handleCloseRight = React.useCallback(
    (keepId: string) => {
      const keepIndex = tabs.findIndex((t) => t.id === keepId);
      for (let i = tabs.length - 1; i > keepIndex; i--) {
        onClose?.(tabs[i].id);
      }
    },
    [onClose, tabs],
  );

  const defaultContextMenu = React.useMemo(
    () => (tabId: string) => (
      <>
        <ContextMenuItem
          onSelect={() => handleClose(tabId)}
          shortcut="Ctrl+W"
        >
          Close
        </ContextMenuItem>
        {tabs.length > 1 && (
          <>
            <ContextMenuItem
              onSelect={() => handleCloseOthers(tabId)}
            >
              Close Others
            </ContextMenuItem>
            <ContextMenuItem
              onSelect={() => handleCloseRight(tabId)}
            >
              Close to the Right
            </ContextMenuItem>
          </>
        )}
      </>
    ),
    [handleClose, handleCloseOthers, handleCloseRight, tabs.length],
  );

  return (
    <>
      <div
        className={cn(
          "flex h-[var(--u-size-tabbar)] shrink-0 items-end border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2",
          className,
        )}
      >
        <div
          className="flex min-w-0 flex-1 items-end overflow-x-auto"
          role="tablist"
        >
          {tabs.map((tab, index) => {
          const active = tab.id === activeId;
          const isDragging = dragRef.current?.index === index;
          const showDropIndicator = dragOverIndex === index && dragRef.current?.index !== index;

          const tabNode = (
            <div
              className={cn(
                "group flex h-[30px] min-w-[120px] max-w-[220px] items-center gap-2 rounded-t-[var(--u-radius-sm)] border border-transparent px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors duration-150",
                active
                  ? "border-[var(--u-color-border)] border-b-[var(--u-color-surface)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                  : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
                isDragging && "opacity-40",
              )}
              draggable={tab.draggable !== false}
              onDragEnd={handleDragEnd}
              onDragOver={(e) => handleDragOver(e, index)}
              onDragStart={(e) => handleDragStart(e, tab.id, index)}
              onDrop={(e) => handleDrop(e, index)}
              style={{ order: index }}
            >
              {onReorder && (
                <span
                  className="shrink-0 cursor-grab text-[var(--u-color-text-soft)] opacity-0 hover:opacity-100 focus-visible:opacity-100 [>.group:hover>&]:opacity-60"
                  onDragStart={(e) => e.stopPropagation()}
                >
                  <GripVertical size={12} />
                </span>
              )}
              <button
                aria-selected={active}
                className="flex min-w-0 flex-1 items-center gap-2 focus-visible:outline-none"
                onClick={() => onSelect(tab.id)}
                role="tab"
                type="button"
              >
                <span className="min-w-0 flex-1 truncate text-left">
                  {tab.modified && (
                    <span className="mr-0.5 inline-block h-2 w-2 rounded-full bg-[var(--u-color-primary)]" title="Unsaved changes" />
                  )}
                  {tab.title}
                </span>
                {tab.loading && (
                  <span className="h-1.5 w-1.5 rounded-full bg-[var(--u-color-primary)] animate-pulse" />
                )}
                {tab.meta}
              </button>
              {onClose && (
                <button
                  aria-label={`Close ${tab.title}`}
                  className="grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
                  onClick={(event) => {
                    event.stopPropagation();
                    handleClose(tab.id);
                  }}
                  title={`Close ${tab.title}`}
                  type="button"
                >
                  <X size={12} />
                </button>
              )}
              {showDropIndicator && (
                <div className="absolute inset-y-0 -right-px w-px bg-[var(--u-color-primary)]" />
              )}
            </div>
          );

          return (
            <ContextMenu key={tab.id}>
              <ContextMenuTrigger asChild>{tabNode}</ContextMenuTrigger>
              <ContextMenuContent>
                {tab.contextMenu ?? defaultContextMenu(tab.id)}
                {tab.contextMenu && onAction && (
                  <ContextMenuSeparator />
                )}
                {onAction && (
                  <ContextMenuItem onSelect={() => onAction({ close: tab.id })} shortcut="Ctrl+W">
                    Close
                  </ContextMenuItem>
                )}
              </ContextMenuContent>
            </ContextMenu>
          );
        })}
        </div>
        {endControl && (
          <div className="ml-auto shrink-0 flex items-center self-center pr-2">{endControl}</div>
        )}
      </div>

      {/* Close confirmation popover */}
      {confirmTabId && confirmResolve && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center"
          onClick={() => confirmResolve(false)}
        >
          <div className="fixed inset-0 bg-black/30" />
          <div
            className="relative z-10 w-72 rounded-[var(--u-radius-lg)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <p className="text-[13px] font-medium text-[var(--u-color-text)]">
              Unsaved changes
            </p>
            <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
              Close this tab without saving?
            </p>
            <div className="mt-3 flex justify-end gap-2">
              <button
                className="h-[var(--u-size-button)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 text-[12px] text-[var(--u-color-text)] hover:bg-[var(--u-color-surface-hover)]"
                onClick={() => confirmResolve(false)}
              >
                Cancel
              </button>
              <button
                className="h-[var(--u-size-button)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-danger)] bg-[var(--u-color-danger-soft)] px-3 text-[12px] text-[var(--u-color-danger)] hover:bg-[var(--u-color-danger-soft)]"
                onClick={() => confirmResolve(true)}
              >
                Discard
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
