import * as React from "react";
import { ChevronRight, Loader2 } from "lucide-react";

import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "./menus";
import {
  TREE_DRAG_TYPE,
  TREE_DROP_POSITIONS,
  resolveRowDropTarget,
  type TreeViewDragTarget,
  type TreeViewDropPosition,
  type TreeViewItem,
} from "./tree-view-model";
import { cn } from "./utils";

export function TreeRow({
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
        aria-busy={item.loading || undefined}
        className="grid h-4 w-4 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
        disabled={!hasChildren && !item.loading}
        onClick={() => hasChildren && toggle(item.id)}
        tabIndex={-1}
        type="button"
      >
        {item.loading ? (
          <Loader2 className="animate-spin" size={12} />
        ) : hasChildren ? (
          <ChevronRight className={cn("transition-transform", expanded && "rotate-90")} size={12} />
        ) : null}
      </button>
      {item.icon && <span className="grid h-4 w-4 shrink-0 place-items-center">{item.icon}</span>}
      <button
        className={cn(
          "min-w-0 flex-1 select-none truncate text-left disabled:cursor-not-allowed",
          draggable && "cursor-grab touch-none active:cursor-grabbing",
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
