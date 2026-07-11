import * as React from "react";
import { cn } from "./utils";
import { TreeRow } from "./tree-view-row";
import {
  TREE_DRAG_TYPE,
  escapeId,
  flatten,
  resolveRowDropTarget,
  searchText,
  type PointerDragState,
  type TreeViewDragTarget,
  type TreeViewDropEvent,
  type TreeViewDropPosition,
  type TreeViewItem,
} from "./tree-view-model";

export type { TreeViewDropEvent, TreeViewDropPosition, TreeViewItem } from "./tree-view-model";

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
      case "F10":
      case "ContextMenu":
        if ((event.shiftKey || event.key === "ContextMenu") && node && !node.item.disabled) {
          event.preventDefault();
          const el = containerRef.current?.querySelector<HTMLElement>(
            `[data-tree-id="${escapeId(node.item.id)}"]`,
          );
          el?.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
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
