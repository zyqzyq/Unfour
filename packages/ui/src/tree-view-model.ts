import type * as React from "react";

export const TREE_DRAG_TYPE = "application/x-unfour-tree-item";
export const TREE_DROP_POSITIONS: TreeViewDropPosition[] = ["inside", "before", "after"];

export type TreeViewItem = {
  actions?: React.ReactNode;
  children?: TreeViewItem[];
  contextMenu?: React.ReactNode;
  disabled?: boolean;
  /** Show a loading spinner on the expand arrow (for async child loading). */
  loading?: boolean;
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

export type FlatNode = {
  expanded: boolean;
  hasChildren: boolean;
  item: TreeViewItem;
  level: number;
  parentId: string | null;
};

export type PointerDragState = {
  active: boolean;
  pointerId: number;
  source: TreeViewItem;
  startX: number;
  startY: number;
};

export type TreeViewDragTarget = {
  id: string;
  position: TreeViewDropPosition;
};

export function flatten(
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

export function searchText(item: TreeViewItem): string {
  const source = typeof item.label === "string" ? item.label : item.title ?? "";
  return source.toLowerCase();
}

export function escapeId(id: string): string {
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

export function resolveRowDropTarget(
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
