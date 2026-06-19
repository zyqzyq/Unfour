import * as React from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "./utils";
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "./menus";

export type TreeViewItem = {
  actions?: React.ReactNode;
  children?: TreeViewItem[];
  contextMenu?: React.ReactNode;
  disabled?: boolean;
  icon?: React.ReactNode;
  id: string;
  label: React.ReactNode;
  meta?: React.ReactNode;
  title?: string;
};

export function TreeView({
  className,
  defaultExpandedIds = [],
  items,
  onSelect,
  selectedId,
}: {
  className?: string;
  defaultExpandedIds?: string[];
  items: TreeViewItem[];
  onSelect?: (item: TreeViewItem) => void;
  selectedId?: string | null;
}) {
  const [expandedIds, setExpandedIds] = React.useState(() => new Set(defaultExpandedIds));

  function toggle(id: string) {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  return (
    <div className={cn("space-y-0.5", className)} role="tree">
      {items.map((item) => (
        <TreeRow
          expandedIds={expandedIds}
          item={item}
          key={item.id}
          level={0}
          onSelect={onSelect}
          selectedId={selectedId}
          toggle={toggle}
        />
      ))}
    </div>
  );
}

function TreeRow({
  expandedIds,
  item,
  level,
  onSelect,
  selectedId,
  toggle,
}: {
  expandedIds: Set<string>;
  item: TreeViewItem;
  level: number;
  onSelect?: (item: TreeViewItem) => void;
  selectedId?: string | null;
  toggle: (id: string) => void;
}) {
  const hasChildren = Boolean(item.children?.length);
  const expanded = expandedIds.has(item.id);
  const row = (
    <div
      aria-expanded={hasChildren ? expanded : undefined}
      className={cn(
        "group flex h-[var(--u-size-sidebar-row)] min-w-0 items-center gap-1 rounded-[var(--u-radius-sm)] px-1 text-[12px] text-[var(--u-color-text-muted)]",
        selectedId === item.id &&
          "bg-[var(--u-color-primary-soft)] font-semibold text-[var(--u-color-primary)]",
        item.disabled ? "opacity-60" : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
      )}
      role="treeitem"
      style={{ paddingLeft: `${4 + level * 14}px` }}
      title={item.title}
    >
      <button
        aria-label={expanded ? "Collapse" : "Expand"}
        className="grid h-5 w-5 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
        disabled={!hasChildren}
        onClick={() => hasChildren && toggle(item.id)}
        type="button"
      >
        {hasChildren && (
          <ChevronRight className={cn("transition-transform", expanded && "rotate-90")} size={13} />
        )}
      </button>
      {item.icon && <span className="grid h-5 w-5 shrink-0 place-items-center">{item.icon}</span>}
      <button
        className="min-w-0 flex-1 truncate text-left disabled:cursor-not-allowed"
        disabled={item.disabled}
        onClick={() => onSelect?.(item)}
        type="button"
      >
        {item.label}
      </button>
      {item.meta && <span className="shrink-0">{item.meta}</span>}
      {item.actions && <span className="shrink-0 opacity-80">{item.actions}</span>}
    </div>
  );

  return (
    <>
      {item.contextMenu ? (
        <ContextMenu>
          <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
          <ContextMenuContent>{item.contextMenu}</ContextMenuContent>
        </ContextMenu>
      ) : (
        row
      )}
      {hasChildren && expanded && (
        <div role="group">
          {item.children?.map((child) => (
            <TreeRow
              expandedIds={expandedIds}
              item={child}
              key={child.id}
              level={level + 1}
              onSelect={onSelect}
              selectedId={selectedId}
              toggle={toggle}
            />
          ))}
        </div>
      )}
    </>
  );
}
