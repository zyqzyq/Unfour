import type { DatabaseTreeModel } from "./database-tree";
import type { DatabaseQueryWorkspaceTab } from "./types";

export function normalizeQueryContext(
  current: Pick<DatabaseQueryWorkspaceTab, "catalog" | "schema">,
  treeModel: DatabaseTreeModel,
  defaultCatalog: string | null = null,
) {
  const explicitCatalog = current.catalog?.trim() || null;
  const configuredCatalog = defaultCatalog?.trim() || null;
  const lookupCatalog = explicitCatalog ?? configuredCatalog;
  const catalogNode =
    lookupCatalog !== null
      ? treeModel.catalogs.find((catalog) => catalog.key === lookupCatalog)
      : treeModel.catalogs.length === 1 && treeModel.catalogs[0]?.key === ""
        ? treeModel.catalogs[0]
        : null;

  // A query without an explicit catalog is valid: the connection's default
  // database remains the server-side fallback. Do not silently bind a new
  // query to the first catalog returned by the server.
  if (!catalogNode) {
    return { catalog: explicitCatalog, schema: explicitCatalog ? current.schema : null };
  }

  if (!catalogNode.hasSchemaLevel) {
    return { catalog: explicitCatalog, schema: null };
  }

  const currentSchema = catalogNode.schemas.find((schema) => schema.key === (current.schema ?? ""));
  const fallbackSchema = currentSchema ?? catalogNode.schemas[0];
  return {
    catalog: explicitCatalog,
    schema: fallbackSchema?.key || null,
  };
}

