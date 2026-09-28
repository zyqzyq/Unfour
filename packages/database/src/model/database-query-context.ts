import type { DatabaseTreeModel } from "./database-tree";
import type { DatabaseQueryWorkspaceTab } from "./types";

export function normalizeQueryContext(
  current: Pick<DatabaseQueryWorkspaceTab, "catalog" | "schema">,
  _treeModel: DatabaseTreeModel,
  defaultCatalog: string | null = null,
) {
  // Tree contents are discovery data, never authority to replace a query's
  // namespace. An empty/partial tree cannot invalidate an explicit schema.
  return {
    catalog: current.catalog?.trim() || defaultCatalog?.trim() || null,
    schema: current.schema?.trim() || null,
  };
}
