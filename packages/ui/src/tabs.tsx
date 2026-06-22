import * as React from "react";
import { X } from "lucide-react";
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "./menus";
import { cn } from "./utils";

export type WorkspaceTab = {
  /** Right-click menu content (ContextMenuItem nodes) for this tab. */
  contextMenu?: React.ReactNode;
  id: string;
  loading?: boolean;
  meta?: React.ReactNode;
  modified?: boolean;
  title: string;
};

export function Tabs({
  activeId,
  className,
  onClose,
  onSelect,
  tabs,
}: {
  activeId: string;
  className?: string;
  onClose?: (tabId: string) => void;
  onSelect: (tabId: string) => void;
  tabs: WorkspaceTab[];
}) {
  return (
    <div
      className={cn(
        "flex h-[var(--u-size-tabbar)] shrink-0 items-end overflow-x-auto border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2",
        className,
      )}
      role="tablist"
    >
      {tabs.map((tab) => {
        const active = tab.id === activeId;
        const tabNode = (
          <div
            className={cn(
              "group flex h-[30px] min-w-[120px] max-w-[220px] items-center gap-2 rounded-t-[var(--u-radius-sm)] border border-transparent px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors",
              active
                ? "border-[var(--u-color-border)] border-b-[var(--u-color-surface)] bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
            )}
          >
            <button
              aria-selected={active}
              className="flex min-w-0 flex-1 items-center gap-2 focus-visible:outline-none"
              onClick={() => onSelect(tab.id)}
              role="tab"
              type="button"
            >
              <span className="min-w-0 flex-1 truncate text-left">
                {tab.modified ? "* " : ""}
                {tab.title}
              </span>
              {tab.loading && (
                <span className="h-1.5 w-1.5 rounded-full bg-[var(--u-color-primary)]" />
              )}
              {tab.meta}
            </button>
            {onClose && (
              <button
                aria-label={`Close ${tab.title}`}
                className="grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
                onClick={(event) => {
                  event.stopPropagation();
                  onClose(tab.id);
                }}
                title={`Close ${tab.title}`}
                type="button"
              >
                <X size={12} />
              </button>
            )}
          </div>
        );

        if (!tab.contextMenu) {
          return <React.Fragment key={tab.id}>{tabNode}</React.Fragment>;
        }

        return (
          <ContextMenu key={tab.id}>
            <ContextMenuTrigger asChild>{tabNode}</ContextMenuTrigger>
            <ContextMenuContent>{tab.contextMenu}</ContextMenuContent>
          </ContextMenu>
        );
      })}
    </div>
  );
}
