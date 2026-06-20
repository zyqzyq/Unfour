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

type FlatNode = {
  expanded: boolean;
  hasChildren: boolean;
  item: TreeViewItem;
  level: number;
  parentId: string | null;
};

function flatten(
  items: TreeViewItem[],
  expandedIds: Set<string>,
  level = 0,
  parentId: string | null = null,
  out: FlatNode[] = [],
): FlatNode[] {
  for (const item of items) {
    const hasChildren = Boolean(item.children?.length);
    const expanded = hasChildren && expandedIds.has(item.id);
    out.push({ expanded, hasChildren, item, level, parentId });
    if (expanded && item.children) {
      flatten(item.children, expandedIds, level + 1, item.id, out);
    }
  }
  return out;
}

function searchText(item: TreeViewItem): string {
  const source = typeof item.label === "string" ? item.label : item.title ?? "";
  return source.toLowerCase();
}

function escapeId(id: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(id);
  }
  return id.replace(/["\\]/g, "\\$&");
}

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
  const [focusedId, setFocusedId] = React.useState<string | null>(null);
  const containerRef = React.useRef<HTMLDivElement>(null);
  const typeAheadRef = React.useRef<{ buffer: string; at: number }>({ buffer: "", at: 0 });

  const flat = React.useMemo(() => flatten(items, expandedIds), [items, expandedIds]);

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

  // The single tab stop is the focused row, falling back to the selected row,
  // then the first enabled row, so the tree is reachable with one Tab press.
  const rovingId =
    (focusedId && flat.some((node) => node.item.id === focusedId) && focusedId) ||
    (selectedId && flat.some((node) => node.item.id === selectedId) && selectedId) ||
    flat.find((node) => !node.item.disabled)?.item.id ||
    null;

  function focusId(id: string) {
    setFocusedId(id);
    const el = containerRef.current?.querySelector<HTMLElement>(
      `[data-tree-id="${escapeId(id)}"]`,
    );
    el?.focus();
  }

  function focusIndex(index: number) {
    const node = flat[index];
    if (node) {
      focusId(node.item.id);
    }
  }

  function step(fromIndex: number, direction: 1 | -1) {
    for (let i = fromIndex + direction; i >= 0 && i < flat.length; i += direction) {
      if (!flat[i].item.disabled) {
        focusIndex(i);
        return;
      }
    }
  }

  function typeAhead(char: string) {
    const now = Date.now();
    const state = typeAheadRef.current;
    state.buffer = now - state.at > 600 ? char.toLowerCase() : state.buffer + char.toLowerCase();
    state.at = now;
    const currentIndex = flat.findIndex((node) => node.item.id === focusedId);
    for (let offset = 1; offset <= flat.length; offset += 1) {
      const node = flat[(currentIndex + offset + flat.length) % flat.length];
      if (!node.item.disabled && searchText(node.item).startsWith(state.buffer)) {
        focusId(node.item.id);
        return;
      }
    }
  }

  function onKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (!flat.length) {
      return;
    }
    const currentIndex = flat.findIndex((node) => node.item.id === focusedId);
    const node = currentIndex >= 0 ? flat[currentIndex] : null;

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        step(currentIndex, 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        step(currentIndex, -1);
        break;
      case "Home": {
        event.preventDefault();
        const first = flat.findIndex((candidate) => !candidate.item.disabled);
        if (first >= 0) focusIndex(first);
        break;
      }
      case "End": {
        event.preventDefault();
        for (let i = flat.length - 1; i >= 0; i -= 1) {
          if (!flat[i].item.disabled) {
            focusIndex(i);
            break;
          }
        }
        break;
      }
      case "ArrowRight":
        if (!node) break;
        event.preventDefault();
        if (node.hasChildren && !node.expanded) {
          toggle(node.item.id);
        } else if (node.hasChildren && node.expanded) {
          step(currentIndex, 1);
        }
        break;
      case "ArrowLeft":
        if (!node) break;
        event.preventDefault();
        if (node.hasChildren && node.expanded) {
          toggle(node.item.id);
        } else if (node.parentId) {
          focusId(node.parentId);
        }
        break;
      case "Enter":
        if (node && !node.item.disabled) {
          event.preventDefault();
          onSelect?.(node.item);
        }
        break;
      case " ":
        if (node && !node.item.disabled) {
          event.preventDefault();
          onSelect?.(node.item);
        }
        break;
      default:
        if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
          typeAhead(event.key);
        }
    }
  }

  return (
    <div
      className={cn("space-y-0.5", className)}
      onKeyDown={onKeyDown}
      ref={containerRef}
      role="tree"
    >
      {items.map((item) => (
        <TreeRow
          expandedIds={expandedIds}
          item={item}
          key={item.id}
          level={0}
          onFocusRow={setFocusedId}
          onSelect={onSelect}
          rovingId={rovingId}
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
  onFocusRow,
  onSelect,
  rovingId,
  selectedId,
  toggle,
}: {
  expandedIds: Set<string>;
  item: TreeViewItem;
  level: number;
  onFocusRow: (id: string) => void;
  onSelect?: (item: TreeViewItem) => void;
  rovingId: string | null;
  selectedId?: string | null;
  toggle: (id: string) => void;
}) {
  const hasChildren = Boolean(item.children?.length);
  const expanded = expandedIds.has(item.id);
  const row = (
    <div
      aria-disabled={item.disabled || undefined}
      aria-expanded={hasChildren ? expanded : undefined}
      aria-selected={selectedId === item.id || undefined}
      className={cn(
        "group flex h-[var(--u-size-sidebar-row)] min-w-0 items-center gap-1 rounded-[var(--u-radius-sm)] px-1 text-[12px] text-[var(--u-color-text-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]",
        selectedId === item.id &&
          "bg-[var(--u-color-primary-soft)] font-semibold text-[var(--u-color-primary)]",
        item.disabled ? "opacity-60" : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
      )}
      data-tree-id={item.id}
      onFocus={() => onFocusRow(item.id)}
      role="treeitem"
      style={{ paddingLeft: `${4 + level * 14}px` }}
      tabIndex={rovingId === item.id ? 0 : -1}
      title={item.title}
    >
      <button
        aria-label={expanded ? "Collapse" : "Expand"}
        className="grid h-5 w-5 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
        disabled={!hasChildren}
        onClick={() => hasChildren && toggle(item.id)}
        tabIndex={-1}
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
        tabIndex={-1}
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
              onFocusRow={onFocusRow}
              onSelect={onSelect}
              rovingId={rovingId}
              selectedId={selectedId}
              toggle={toggle}
            />
          ))}
        </div>
      )}
    </>
  );
}
