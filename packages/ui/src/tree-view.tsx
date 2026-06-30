import * as React from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "./utils";
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "./menus";

const TREE_DRAG_TYPE = "application/x-unfour-tree-item";
const TREE_DROP_POSITIONS: TreeViewDropPosition[] = ["inside", "before", "after"];

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

export type TreeViewDropEvent = {
  position: TreeViewDropPosition;
  source: TreeViewItem;
  target: TreeViewItem;
};

export type TreeViewDropPosition = "before" | "inside" | "after";

type FlatNode = {
  expanded: boolean;
  hasChildren: boolean;
  item: TreeViewItem;
  level: number;
  parentId: string | null;
};

type PointerDragState = {
  active: boolean;
  pointerId: number;
  source: TreeViewItem;
  startX: number;
  startY: number;
};

type TreeViewDragTarget = {
  id: string;
  position: TreeViewDropPosition;
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

function preferredDropPosition(
  row: HTMLElement,
  clientY: number,
): TreeViewDropPosition {
  const rect = row.getBoundingClientRect();
  if (!rect.height) {
    if (clientY > 0 && clientY <= 8) {
      return "before";
    }
    return "inside";
  }
  const offset = clientY - rect.top;
  const threshold = Math.min(8, rect.height * 0.35);
  if (offset <= threshold) {
    return "before";
  }
  if (offset >= rect.height - threshold) {
    return "after";
  }
  return "inside";
}

function dropPositionFallbacks(
  preferred: TreeViewDropPosition,
  row: HTMLElement,
  clientY: number,
): TreeViewDropPosition[] {
  if (preferred === "before") {
    return ["before", "inside", "after"];
  }
  if (preferred === "after") {
    return ["after", "inside", "before"];
  }
  const rect = row.getBoundingClientRect();
  const edge = clientY < rect.top + rect.height / 2 ? "before" : "after";
  return ["inside", edge, edge === "before" ? "after" : "before"];
}

function resolveRowDropTarget(
  source: TreeViewItem,
  target: TreeViewItem,
  row: HTMLElement,
  clientY: number,
  canDropOn: (
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) => boolean,
): TreeViewDragTarget | null {
  for (const position of dropPositionFallbacks(
    preferredDropPosition(row, clientY),
    row,
    clientY,
  )) {
    if (canDropOn(source, target, position)) {
      return { id: target.id, position };
    }
  }
  return null;
}

export function TreeView({
  canDrag,
  canDrop,
  className,
  defaultExpandedIds = [],
  items,
  onActivate,
  onDrop,
  onSelect,
  onToggle,
  selectedId,
}: {
  canDrag?: (item: TreeViewItem) => boolean;
  canDrop?: (
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) => boolean;
  className?: string;
  defaultExpandedIds?: string[];
  items: TreeViewItem[];
  /** Fired on double-click of a row label (e.g. "double-click to connect"). */
  onActivate?: (item: TreeViewItem) => void;
  onDrop?: (event: TreeViewDropEvent) => void;
  onSelect?: (item: TreeViewItem) => void;
  /** Fired when a node is expanded or collapsed (e.g. to lazy-load children). */
  onToggle?: (id: string, expanded: boolean) => void;
  selectedId?: string | null;
}) {
  const [expandedIds, setExpandedIds] = React.useState(() => new Set(defaultExpandedIds));
  const [focusedId, setFocusedId] = React.useState<string | null>(null);
  const [dragSource, setDragSource] = React.useState<TreeViewItem | null>(null);
  const [dragTarget, setDragTarget] = React.useState<TreeViewDragTarget | null>(null);
  const containerRef = React.useRef<HTMLDivElement>(null);
  const dragSourceRef = React.useRef<TreeViewItem | null>(null);
  const pointerDragRef = React.useRef<PointerDragState | null>(null);
  const suppressClickRef = React.useRef<string | null>(null);
  const typeAheadRef = React.useRef<{ buffer: string; at: number }>({ buffer: "", at: 0 });

  // Auto-expand ids already applied. Lets lazily-loaded content (e.g. a database
  // schema fetched on expand) auto-expand as new defaults appear, without
  // re-expanding nodes the user has since collapsed. Without this the tree only
  // honors defaultExpandedIds at mount, so content loaded later stays hidden.
  const appliedDefaultsRef = React.useRef<Set<string>>(new Set(defaultExpandedIds));
  const defaultsKey = defaultExpandedIds.join("|");
  React.useEffect(() => {
    const applied = appliedDefaultsRef.current;
    const added = defaultExpandedIds.filter((id) => !applied.has(id));
    if (!added.length) {
      return;
    }
    for (const id of added) {
      applied.add(id);
    }
    setExpandedIds((current) => {
      const next = new Set(current);
      for (const id of added) {
        next.add(id);
      }
      return next;
    });
    // Auto-expansion must drive the same lazy load as a manual expand, so
    // consumers fetch children that only appear once a node is opened. The
    // appliedDefaultsRef guard above makes this a no-op when onToggle's identity
    // changes without new defaults, so listing it in deps cannot double-fire.
    for (const id of added) {
      onToggle?.(id, true);
    }
    // defaultsKey captures the meaningful change in defaultExpandedIds contents.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [defaultsKey, onToggle]);

  const flat = React.useMemo(() => flatten(items, expandedIds), [items, expandedIds]);
  const itemById = React.useMemo(
    () => new Map(flat.map((node) => [node.item.id, node.item])),
    [flat],
  );

  function toggle(id: string) {
    // Derive the next state from current expandedIds rather than from inside the
    // updater: React runs the updater during render, after this function returns,
    // so reading a flag set inside it would always see the stale initial value
    // and report the wrong expanded state to onToggle (breaking lazy loading).
    const nowExpanded = !expandedIds.has(id);
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
    onToggle?.(id, nowExpanded);
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

  function canDropOn(
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) {
    if (!onDrop || source.id === target.id || target.disabled) {
      return false;
    }
    return canDrop ? canDrop(source, target, position) : true;
  }

  function setDragSourceItem(item: TreeViewItem | null) {
    dragSourceRef.current = item;
    setDragSource(item);
  }

  function dragSourceFromEvent(event: React.DragEvent<HTMLDivElement>) {
    if (dragSourceRef.current) {
      return dragSourceRef.current;
    }
    const sourceId =
      event.dataTransfer.getData(TREE_DRAG_TYPE) ||
      event.dataTransfer.getData("text/plain");
    return sourceId ? (itemById.get(sourceId) ?? null) : null;
  }

  function rowFromPoint(clientX: number, clientY: number) {
    const target = document.elementFromPoint(clientX, clientY);
    return target?.closest<HTMLElement>("[data-tree-id]") ?? null;
  }

  function itemFromRow(row: HTMLElement | null) {
    const id = row?.dataset.treeId;
    return id ? (itemById.get(id) ?? null) : null;
  }

  function dropTargetFromPoint(
    source: TreeViewItem,
    clientX: number,
    clientY: number,
  ) {
    const row = rowFromPoint(clientX, clientY);
    const target = itemFromRow(row);
    if (!row || !target) {
      return null;
    }
    return resolveRowDropTarget(source, target, row, clientY, canDropOn);
  }

  function startPointerDrag(
    item: TreeViewItem,
    event: React.PointerEvent<HTMLElement>,
  ) {
    if (event.button !== 0 || item.disabled) {
      return;
    }
    pointerDragRef.current = {
      active: false,
      pointerId: event.pointerId,
      source: item,
      startX: event.clientX,
      startY: event.clientY,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function updatePointerDrag(event: React.PointerEvent<HTMLElement>) {
    const state = pointerDragRef.current;
    if (!state || state.pointerId !== event.pointerId) {
      return;
    }
    const moved =
      Math.abs(event.clientX - state.startX) > 4 ||
      Math.abs(event.clientY - state.startY) > 4;
    if (!moved && !state.active) {
      return;
    }
    state.active = true;
    setDragSourceItem(state.source);
    event.preventDefault();
    setDragTarget(dropTargetFromPoint(state.source, event.clientX, event.clientY));
  }

  function endPointerDrag(event: React.PointerEvent<HTMLElement>) {
    const state = pointerDragRef.current;
    if (!state || state.pointerId !== event.pointerId) {
      return;
    }
    pointerDragRef.current = null;
    event.currentTarget.releasePointerCapture?.(event.pointerId);
    if (state.active) {
      event.preventDefault();
      event.stopPropagation();
      suppressClickRef.current = state.source.id;
      const row = rowFromPoint(event.clientX, event.clientY);
      const target = itemFromRow(row);
      const dropTarget =
        row && target
          ? resolveRowDropTarget(state.source, target, row, event.clientY, canDropOn)
          : null;
      if (target && dropTarget) {
        onDrop?.({ position: dropTarget.position, source: state.source, target });
      }
    }
    clearDrag();
  }

  function cancelPointerDrag(event: React.PointerEvent<HTMLElement>) {
    const state = pointerDragRef.current;
    if (!state || state.pointerId !== event.pointerId) {
      return;
    }
    pointerDragRef.current = null;
    clearDrag();
  }

  function consumeSuppressedClick(item: TreeViewItem) {
    if (suppressClickRef.current !== item.id) {
      return false;
    }
    suppressClickRef.current = null;
    return true;
  }

  function clearDrag() {
    dragSourceRef.current = null;
    setDragSource(null);
    setDragTarget(null);
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
          canDrag={canDrag}
          canDropOn={canDropOn}
          dragSource={dragSource}
          dragTarget={dragTarget}
          expandedIds={expandedIds}
          item={item}
          key={item.id}
          level={0}
          onActivate={onActivate}
          onClearDrag={clearDrag}
          onDropItem={(source, target, position) =>
            onDrop?.({ position, source, target })
          }
          onFocusRow={setFocusedId}
          onGetDragSource={dragSourceFromEvent}
          onCancelPointerDrag={cancelPointerDrag}
          onConsumeSuppressedClick={consumeSuppressedClick}
          onSelect={onSelect}
          onStartPointerDrag={startPointerDrag}
          onUpdatePointerDrag={updatePointerDrag}
          onEndPointerDrag={endPointerDrag}
          onSetDragSource={setDragSourceItem}
          onSetDragTarget={setDragTarget}
          rovingId={rovingId}
          selectedId={selectedId}
          toggle={toggle}
        />
      ))}
    </div>
  );
}

function TreeRow({
  canDrag,
  canDropOn,
  dragSource,
  dragTarget,
  expandedIds,
  item,
  level,
  onActivate,
  onClearDrag,
  onDropItem,
  onFocusRow,
  onGetDragSource,
  onCancelPointerDrag,
  onConsumeSuppressedClick,
  onSelect,
  onStartPointerDrag,
  onUpdatePointerDrag,
  onEndPointerDrag,
  onSetDragSource,
  onSetDragTarget,
  rovingId,
  selectedId,
  toggle,
}: {
  canDrag?: (item: TreeViewItem) => boolean;
  canDropOn: (
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) => boolean;
  dragSource: TreeViewItem | null;
  dragTarget: TreeViewDragTarget | null;
  expandedIds: Set<string>;
  item: TreeViewItem;
  level: number;
  onActivate?: (item: TreeViewItem) => void;
  onClearDrag: () => void;
  onDropItem: (
    source: TreeViewItem,
    target: TreeViewItem,
    position: TreeViewDropPosition,
  ) => void;
  onFocusRow: (id: string) => void;
  onGetDragSource: (event: React.DragEvent<HTMLDivElement>) => TreeViewItem | null;
  onCancelPointerDrag: (event: React.PointerEvent<HTMLElement>) => void;
  onConsumeSuppressedClick: (item: TreeViewItem) => boolean;
  onSelect?: (item: TreeViewItem) => void;
  onStartPointerDrag: (
    item: TreeViewItem,
    event: React.PointerEvent<HTMLElement>,
  ) => void;
  onUpdatePointerDrag: (event: React.PointerEvent<HTMLElement>) => void;
  onEndPointerDrag: (event: React.PointerEvent<HTMLElement>) => void;
  onSetDragSource: (item: TreeViewItem | null) => void;
  onSetDragTarget: (target: TreeViewDragTarget | null) => void;
  rovingId: string | null;
  selectedId?: string | null;
  toggle: (id: string) => void;
}) {
  const hasChildren = Boolean(item.children?.length);
  const expanded = expandedIds.has(item.id);
  const draggable = !item.disabled && Boolean(canDrag?.(item));
  const canReceiveDrop = dragSource
    ? TREE_DROP_POSITIONS.some((position) => canDropOn(dragSource, item, position))
    : false;
  const dropPosition = canReceiveDrop && dragTarget?.id === item.id ? dragTarget.position : null;
  const isDropInside = dropPosition === "inside";
  const isDropBefore = dropPosition === "before";
  const isDropAfter = dropPosition === "after";
  const isDragSource = dragSource?.id === item.id;

  function handleDragStart(event: React.DragEvent<HTMLElement>) {
    if (!draggable) {
      event.preventDefault();
      return;
    }
    onSetDragSource(item);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData(TREE_DRAG_TYPE, item.id);
    event.dataTransfer.setData("text/plain", item.id);
  }

  function handleDragOver(event: React.DragEvent<HTMLDivElement>) {
    const source = onGetDragSource(event);
    const target = source
      ? resolveRowDropTarget(source, item, event.currentTarget, event.clientY, canDropOn)
      : null;
    if (!source || !target) {
      if (dragTarget?.id === item.id) {
        onSetDragTarget(null);
      }
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    onSetDragTarget(target);
  }

  function handleDragLeave(event: React.DragEvent<HTMLDivElement>) {
    if (
      event.relatedTarget instanceof Node &&
      event.currentTarget.contains(event.relatedTarget)
    ) {
      return;
    }
    onSetDragTarget(null);
  }

  function handleDrop(event: React.DragEvent<HTMLDivElement>) {
    const source = onGetDragSource(event);
    const target = source
      ? resolveRowDropTarget(source, item, event.currentTarget, event.clientY, canDropOn)
      : null;
    if (!source || !target) {
      return;
    }
    event.preventDefault();
    onDropItem(source, item, target.position);
    onClearDrag();
  }

  const row = (
    <div
      aria-disabled={item.disabled || undefined}
      aria-expanded={hasChildren ? expanded : undefined}
      aria-selected={selectedId === item.id || undefined}
      className={cn(
        "group relative flex h-[var(--u-size-sidebar-row)] min-w-0 items-center gap-0.5 rounded-[var(--u-radius-sm)] px-1 text-[12px] text-[var(--u-color-text-muted)] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]",
        selectedId === item.id &&
          "bg-[var(--u-color-primary-soft)] font-semibold text-[var(--u-color-primary)]",
        isDropInside &&
          "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)] ring-1 ring-inset ring-[var(--u-color-border-strong)]",
        isDragSource && "opacity-40",
        !isDragSource && !item.disabled && !isDropInside &&
          "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
        item.disabled && "opacity-60",
      )}
      data-drop-position={dropPosition ?? undefined}
      data-tree-id={item.id}
      onDragEnd={draggable ? onClearDrag : undefined}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDragStart={draggable ? handleDragStart : undefined}
      onDrop={handleDrop}
      onFocus={() => onFocusRow(item.id)}
      role="treeitem"
      style={{ paddingLeft: `${4 + level * 14}px` }}
      tabIndex={rovingId === item.id ? 0 : -1}
      title={item.title}
    >
      {isDropBefore && <DropLine position="before" />}
      {isDropAfter && <DropLine position="after" />}
      <button
        aria-label={expanded ? "Collapse" : "Expand"}
        className="grid h-4 w-4 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
        disabled={!hasChildren}
        onClick={() => hasChildren && toggle(item.id)}
        tabIndex={-1}
        type="button"
      >
        {hasChildren && (
          <ChevronRight className={cn("transition-transform", expanded && "rotate-90")} size={12} />
        )}
      </button>
      {item.icon && <span className="grid h-4 w-4 shrink-0 place-items-center">{item.icon}</span>}
      <button
        className={cn(
          "min-w-0 flex-1 truncate text-left disabled:cursor-not-allowed",
          draggable && "cursor-grab select-none touch-none active:cursor-grabbing",
        )}
        disabled={item.disabled}
        onClick={(event) => {
          if (onConsumeSuppressedClick(item)) {
            event.preventDefault();
            event.stopPropagation();
            return;
          }
          onSelect?.(item);
        }}
        onDoubleClick={() => onActivate?.(item)}
        onDragStart={draggable ? handleDragStart : undefined}
        onPointerCancel={draggable ? onCancelPointerDrag : undefined}
        onPointerDown={
          draggable ? (event) => onStartPointerDrag(item, event) : undefined
        }
        onPointerMove={draggable ? onUpdatePointerDrag : undefined}
        onPointerUp={draggable ? onEndPointerDrag : undefined}
        tabIndex={-1}
        type="button"
      >
        {item.label}
      </button>
      {item.meta && <span className="shrink-0">{item.meta}</span>}
      {item.actions && <span className="shrink-0">{item.actions}</span>}
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
              canDrag={canDrag}
              canDropOn={canDropOn}
              dragSource={dragSource}
              dragTarget={dragTarget}
              expandedIds={expandedIds}
              item={child}
              key={child.id}
              level={level + 1}
              onActivate={onActivate}
              onClearDrag={onClearDrag}
              onDropItem={onDropItem}
              onFocusRow={onFocusRow}
              onGetDragSource={onGetDragSource}
              onCancelPointerDrag={onCancelPointerDrag}
              onConsumeSuppressedClick={onConsumeSuppressedClick}
              onSelect={onSelect}
              onStartPointerDrag={onStartPointerDrag}
              onUpdatePointerDrag={onUpdatePointerDrag}
              onEndPointerDrag={onEndPointerDrag}
              onSetDragSource={onSetDragSource}
              onSetDragTarget={onSetDragTarget}
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

function DropLine({ position }: { position: "before" | "after" }) {
  return (
    <span
      aria-hidden
      className={cn(
        "pointer-events-none absolute left-1 right-1 z-10 h-0.5 rounded-full bg-[var(--u-color-primary)] shadow-[0_0_0_1px_color-mix(in_srgb,var(--u-color-primary)_24%,transparent)]",
        position === "before" ? "top-0" : "bottom-0",
      )}
    />
  );
}
