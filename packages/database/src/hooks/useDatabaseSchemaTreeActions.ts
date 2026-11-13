import { type Dispatch, type SetStateAction } from "react";
import type { QueryClient } from "@tanstack/react-query";
import {
  getDatabaseSchema,
  listDatabaseCatalogs,
  databaseRouteVersion,
  databaseActionContext,
} from "@unfour/command-client";
import type { DatabaseConnection, DatabaseSchema } from "@unfour/command-client";
import { formatDatabaseError } from "../result-utils";

export function useDatabaseSchemaTreeActions({
  catalogNamesByConn,
  connections,
  queryClient,
  setCatalogNamesByConn,
  setTreeErrors,
  setTreeLoadingKeys,
  setTreeSchemaCache,
  treeLoadingKeys,
  treeSchemaCache,
  workspaceId,
}: {
  catalogNamesByConn: Record<string, string[]>;
  connections?: DatabaseConnection[];
  queryClient: QueryClient;
  setCatalogNamesByConn: Dispatch<SetStateAction<Record<string, string[]>>>;
  setTreeErrors: Dispatch<SetStateAction<Record<string, string>>>;
  setTreeLoadingKeys: Dispatch<SetStateAction<string[]>>;
  setTreeSchemaCache: Dispatch<SetStateAction<Record<string, DatabaseSchema>>>;
  treeLoadingKeys: string[];
  treeSchemaCache: Record<string, DatabaseSchema>;
  workspaceId: string;
}) {
  function setTreeError(key: string, error: unknown) {
    setTreeErrors((prev) => ({ ...prev, [key]: formatDatabaseError(error) }));
  }

  function clearTreeError(key: string) {
    setTreeErrors((prev) => {
      if (!(key in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }

  function loadCatalogNames(connectionId: string, options: { force?: boolean } = {}) {
    const version = databaseRouteVersion(workspaceId, connectionId);
    const context = databaseActionContext(workspaceId, connectionId, connections?.find((c) => c.id === connectionId));
    const key = `names::${connectionId}`;
    if ((!options.force && catalogNamesByConn[connectionId]) || treeLoadingKeys.includes(key)) {
      return;
    }
    setTreeLoadingKeys((current) => [...current, key]);
    queryClient
      .fetchQuery({
        queryKey: ["database-catalogs", workspaceId, connectionId, context],
        queryFn: () => listDatabaseCatalogs(workspaceId, connectionId, context),
      })
      .then((names) => {
        if (version !== databaseRouteVersion(workspaceId, connectionId)) return;
        setCatalogNamesByConn((prev) => ({ ...prev, [connectionId]: names }));
        clearTreeError(key);
      })
      .catch((error) => setTreeError(key, error))
      .finally(() => setTreeLoadingKeys((current) => current.filter((item) => item !== key)));
  }

  // Lazily fetch a database (catalog) schema when its tree node is expanded.
  function loadCatalogSchema(connectionId: string, catalog: string, options: { force?: boolean } = {}) {
    const version = databaseRouteVersion(workspaceId, connectionId);
    const context = databaseActionContext(workspaceId, connectionId, connections?.find((c) => c.id === connectionId));
    const key = `${connectionId}::${catalog}`;
    if ((!options.force && treeSchemaCache[key]) || treeLoadingKeys.includes(key)) {
      return;
    }
    setTreeLoadingKeys((current) => [...current, key]);
    queryClient
      .fetchQuery({
        queryKey: ["database-schema", workspaceId, connectionId, catalog || null, context],
        queryFn: () => getDatabaseSchema(workspaceId, connectionId, catalog || null, context),
      })
      .then((data) => {
        if (version !== databaseRouteVersion(workspaceId, connectionId)) return;
        setTreeSchemaCache((prev) => ({ ...prev, [key]: data }));
        clearTreeError(key);
      })
      .catch((error) => setTreeError(key, error))
      .finally(() => setTreeLoadingKeys((current) => current.filter((item) => item !== key)));
  }

  // Load a connection's databases when its tree node is expanded: SQLite loads
  // its single file schema directly; PostgreSQL/MySQL load the database list.
  function loadConnectionRoot(connection: DatabaseConnection) {
    if (connection.driver === "sqlite") {
      loadCatalogSchema(connection.id, "");
      return;
    }
    loadCatalogNames(connection.id);
  }

  return {
    loadCatalogNames,
    loadCatalogSchema,
    loadConnectionRoot,
  };
}
