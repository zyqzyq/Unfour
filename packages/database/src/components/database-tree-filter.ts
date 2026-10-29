import type { TreeViewItem } from "@unfour/ui";

/** Filter only the already-built tree. Never inspect SQL text or fetch children. */
export function filterDatabaseTree(items: TreeViewItem[], query: string) {
  const needle = query.trim().toLowerCase();
  const expandedIds: string[] = [];
  if (!needle) return { items, expandedIds };

  function visit(nodes: TreeViewItem[], parentMatches = false): TreeViewItem[] {
    return nodes.flatMap((item) => {
      const matches = parentMatches || (!item.disabled && typeof item.label === "string" && item.label.toLowerCase().includes(needle));
      const children = item.children ? visit(item.children, matches) : undefined;
      if (!matches && !children?.length) return [];
      if (children?.length) expandedIds.push(item.id);
      return [{ ...item, children }];
    });
  }

  return { items: visit(items), expandedIds };
}
