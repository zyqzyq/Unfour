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
import { Dialog } from "./dialog-primitives";
import { DialogContent, DialogDescription, DialogTitle } from "./dialog";
import { useI18n } from "./i18n";

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
  activeBackground = "surface",
  activeId,
  className,
  endControl,
  onAction,
  onClose,
  onReorder,
  onSelect,
  tabs,
}: {
  /** Match the immediately adjacent panel or toolbar, not the outer workspace. */
  activeBackground?: "surface" | "subtle" | "terminal";
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
  const { t } = useI18n();
  const dragRef = React.useRef<{ id: string; index: number } | null>(null);
  const [dragIndex, setDragIndex] = React.useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = React.useState<number | null>(null);
  const [pendingClose, setPendingClose] = React.useState<{ ids: string[]; action: TabsAction } | null>(null);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const tabbarRef = React.useRef<HTMLDivElement>(null);

  const completeClose = React.useCallback((ids: string[], action: TabsAction) => {
    if (onAction) onAction(action);
    else ids.forEach((id) => onClose?.(id));
  }, [onAction, onClose]);

  const requestClose = React.useCallback((ids: string[], action: TabsAction) => {
    if (tabs.some((tab) => ids.includes(tab.id) && tab.modified)) {
      returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      setPendingClose({ ids, action });
    } else {
      completeClose(ids, action);
    }
  }, [completeClose, tabs]);

  const handleClose = React.useCallback(
    (tabId: string) => {
      requestClose([tabId], { close: tabId });
    },
    [requestClose],
  );

  const handleDragStart = React.useCallback(
    (e: React.DragEvent, tabId: string, index: number) => {
      if (e.dataTransfer) {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/tab-id", tabId);
      }
      dragRef.current = { id: tabId, index };
      setDragIndex(index);
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
      setDragIndex(null);
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
    setDragIndex(null);
    setDragOverIndex(null);
  }, []);

  const handleCloseOthers = React.useCallback(
    (keepId: string) => {
      requestClose(tabs.filter((tab) => tab.id !== keepId).map((tab) => tab.id), { closeOthers: keepId, keep: keepId });
    },
    [requestClose, tabs],
  );

  const handleCloseRight = React.useCallback(
    (keepId: string) => {
      const keepIndex = tabs.findIndex((t) => t.id === keepId);
      if (keepIndex < 0) return;
      requestClose(tabs.slice(keepIndex + 1).map((tab) => tab.id), { closeRight: keepId, keep: keepId });
    },
    [requestClose, tabs],
  );

  const defaultContextMenu = React.useMemo(
    () => (tabId: string) => (
      <>
        <ContextMenuItem
          onSelect={() => handleClose(tabId)}
        >
          {t("common.tabs.close")}
        </ContextMenuItem>
        {tabs.length > 1 && (
          <>
            <ContextMenuItem
              onSelect={() => handleCloseOthers(tabId)}
            >
              {t("common.tabs.closeOthers")}
            </ContextMenuItem>
            <ContextMenuItem
              disabled={tabs[tabs.length - 1]?.id === tabId}
              onSelect={() => handleCloseRight(tabId)}
            >
              {t("common.tabs.closeRight")}
            </ContextMenuItem>
          </>
        )}
      </>
    ),
    [handleClose, handleCloseOthers, handleCloseRight, t, tabs],
  );

  return (
    <>
      <div
        ref={tabbarRef}
        className={cn(
          "flex h-[var(--u-size-tabbar)] shrink-0 items-end border-b border-[var(--u-color-border)] bg-[var(--u-color-chrome)] px-2",
          className,
        )}
      >
        <div
          className="flex min-w-0 flex-1 translate-y-px items-end overflow-x-auto"
          role="tablist"
        >
          {tabs.map((tab, index) => {
          const active = tab.id === activeId;
          const isDragging = dragIndex === index;
          const showDropIndicator = dragOverIndex === index && dragIndex !== index;

          const tabNode = (
            <div
              className={cn(
                "group relative flex h-[30px] min-w-[120px] max-w-[220px] items-center gap-2 rounded-t-[var(--u-radius-sm)] border border-transparent px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors duration-150",
                active
                  ? cn("border-[var(--u-color-border)]", {
                    surface: "border-b-[var(--u-color-surface)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]",
                    subtle: "border-b-[var(--u-color-surface-subtle)] bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text)]",
                    terminal: "border-b-[var(--u-color-terminal-bg)] bg-[var(--u-color-terminal-bg)] text-[var(--u-color-terminal-text)]",
                  }[activeBackground])
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
                className="flex min-w-0 flex-1 items-center gap-2 rounded-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--u-color-focus)]"
                onClick={() => onSelect(tab.id)}
                role="tab"
                type="button"
              >
                <span className="min-w-0 flex-1 truncate text-left">
                  {tab.modified && (
                    <span className="mr-0.5 inline-block h-2 w-2 rounded-full bg-[var(--u-color-primary)]" title={t("common.tabs.unsaved")} />
                  )}
                  {tab.title}
                </span>
                {tab.loading && (
                  <span className="h-1.5 w-1.5 rounded-full bg-[var(--u-color-primary)] animate-pulse" />
                )}
                {tab.meta}
              </button>
              {(onClose || onAction) && (
                <button
                  aria-label={t("common.tabs.closeLabel", { title: tab.title })}
                  className={cn(
                    "grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-[var(--u-color-focus)]",
                    active && activeBackground === "terminal" && "text-[var(--u-color-terminal-muted)]",
                  )}
                  onClick={(event) => {
                    event.stopPropagation();
                    handleClose(tab.id);
                  }}
                  title={t("common.tabs.closeLabel", { title: tab.title })}
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
                {tab.contextMenu ?? ((onClose || onAction) && defaultContextMenu(tab.id))}
                {tab.contextMenu && onAction && (
                  <ContextMenuSeparator />
                )}
                {tab.contextMenu && onAction && (
                  <ContextMenuItem onSelect={() => handleClose(tab.id)}>
                    {t("common.tabs.close")}
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

      <Dialog open={pendingClose !== null} onOpenChange={(open) => !open && setPendingClose(null)}>
          <DialogContent
            className="w-72 p-4"
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              const target = returnFocusRef.current;
              if (target?.isConnected && !target.closest('[role="menu"]')) target.focus();
              else tabbarRef.current?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus();
            }}
          >
            <DialogTitle className="text-[13px] font-medium text-[var(--u-color-text)]">
              {t("common.tabs.unsaved")}
            </DialogTitle>
            <DialogDescription className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
              {t(pendingClose && pendingClose.ids.length > 1 ? "common.tabs.discardMany" : "common.tabs.discardOne")}
            </DialogDescription>
            <div className="mt-3 flex justify-end gap-2">
              <button
                autoFocus
                type="button"
                className="h-[var(--u-size-button)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 text-[12px] text-[var(--u-color-text)] hover:bg-[var(--u-color-surface-hover)]"
                onClick={() => setPendingClose(null)}
              >
                {t("common.confirm.cancel")}
              </button>
              <button
                type="button"
                className="h-[var(--u-size-button)] rounded-[var(--u-radius-sm)] border border-[var(--u-color-danger)] bg-[var(--u-color-danger-soft)] px-3 text-[12px] text-[var(--u-color-danger)] hover:bg-[var(--u-color-danger-soft)]"
                onClick={() => {
                  if (pendingClose) completeClose(pendingClose.ids, pendingClose.action);
                  setPendingClose(null);
                }}
              >
                {t("common.tabs.discard")}
              </button>
            </div>
          </DialogContent>
      </Dialog>
    </>
  );
}
