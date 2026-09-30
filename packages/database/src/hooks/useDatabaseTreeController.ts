import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { canLoadDatabaseSchema } from "../model/database-connection-state";
import { buildDatabaseTree } from "../model/database-tree";
import type { DatabaseConnectionSessionState } from "../model/types";
import { formatDatabaseError } from "../result-utils";
import { useDatabaseCatalogs } from "./useDatabaseCatalogs";
import { useSchemaTree } from "./useSchemaTree";

type DatabaseTreeSynchronizationOptions = {
  schemaEnabled: boolean;
  selectedConnectionId: string | null;
  setConnectionState: (connectionId: string, patch: Partial<DatabaseConnectionSessionState>) => void;
};

export function useDatabaseTreeController({
  schemaEnabled,
  selectedConnection,
  selectedConnectionId,
  workspaceId,
}: {
  schemaEnabled: boolean;
  selectedConnection: DatabaseConnection | null;
  selectedConnectionId: string | null;
  workspaceId: string;
}) {
  // Per-connection tree data so multiple connections can be browsed at once.
  // catalogNamesByConn: connectionId -> database names (PostgreSQL/MySQL).
  // treeSchemaCache: `${connectionId}::${catalog}` -> that database's schema
  // ("" catalog for SQLite). Both are populated lazily as nodes are expanded;
  // the selected connection's data is fed in from its own queries.
  const [catalogNamesByConn, setCatalogNamesByConn] = useState<Record<string, string[]>>({});
  const [treeSchemaCache, setTreeSchemaCache] = useState<Record<string, DatabaseSchema>>({});
  const [treeLoadingKeys, setTreeLoadingKeys] = useState<string[]>([]);
  const [treeErrors, setTreeErrors] = useState<Record<string, string>>({});
  // The initial schema load fetches the connected database (PostgreSQL) or every
  // database (MySQL, which can list them in one call). Other PostgreSQL databases
  // are fetched lazily on expand and cached in treeSchemaCache.
  const schemaQuery = useSchemaTree({
    connection: selectedConnection,
    connectionId: selectedConnectionId,
    enabled: schemaEnabled,
    workspaceId,
  });
  const catalogsQuery = useDatabaseCatalogs({
    connection: selectedConnection,
    connectionId: selectedConnectionId,
    enabled: schemaEnabled,
    workspaceId,
  });
  const visibleSchema = schemaEnabled ? schemaQuery.data : undefined;
  const treeModel = useMemo(
    () => (visibleSchema ? buildDatabaseTree(visibleSchema.tables) : null),
    [visibleSchema],
  );
  // Keep the existing workspace controller's loader/refresh inputs unchanged.
  return {
    catalogNamesByConn,
    catalogsQuery,
    schemaQuery,
    setCatalogNamesByConn,
    setTreeErrors,
    setTreeLoadingKeys,
    setTreeSchemaCache,
    treeErrors,
    treeLoadingKeys,
    treeModel,
    treeSchemaCache,
    visibleSchema,
  };
}

// Keep synchronization at the page's original effect position: tab selection
// and removed-connection pruning run before schema-driven session updates.
export function useDatabaseTreeSynchronization(
  {
    catalogsQuery, schemaQuery, setCatalogNamesByConn, setTreeSchemaCache,
  }: ReturnType<typeof useDatabaseTreeController>,
  {
    schemaEnabled, selectedConnectionId, setConnectionState,
  }: DatabaseTreeSynchronizationOptions,
) {
  const { t } = useI18n();
  useEffect(() => {
    if (!selectedConnectionId || !schemaEnabled || !schemaQuery.data) {
      return;
    }

    setConnectionState(selectedConnectionId, {
      message: t("database.connection.tableCountLoaded", {
        count: schemaQuery.data.tables.length,
      }),
      status: "connected",
    });
  }, [schemaEnabled, schemaQuery.data, selectedConnectionId, setConnectionState, t]);

  useEffect(() => {
    if (!selectedConnectionId || !schemaEnabled || !schemaQuery.error) {
      return;
    }

    setConnectionState(selectedConnectionId, {
      message: formatDatabaseError(schemaQuery.error),
      status: "failed",
    });
  }, [schemaEnabled, schemaQuery.error, selectedConnectionId, setConnectionState]);

  // Feed the selected connection's database list into the per-connection cache
  // so its tree renders without a manual expand.
  useEffect(() => {
    if (!selectedConnectionId || !catalogsQuery.data) {
      return;
    }
    const names = catalogsQuery.data;
    setCatalogNamesByConn((prev) => ({ ...prev, [selectedConnectionId]: names }));
  }, [selectedConnectionId, catalogsQuery.data, setCatalogNamesByConn]);

  // Feed the selected connection's loaded schema into the cache, grouped by
  // catalog (the connected database for PostgreSQL, every database for MySQL,
  // the file for SQLite under the "" catalog key).
  const selectedSchemaData = schemaQuery.data;
  useEffect(() => {
    if (!selectedConnectionId || !selectedSchemaData) {
      return;
    }
    const grouped = new Map<string, DatabaseTable[]>();
    for (const table of selectedSchemaData.tables) {
      const key = table.catalog ?? "";
      grouped.set(key, [...(grouped.get(key) ?? []), table]);
    }
    setTreeSchemaCache((prev) => {
      const next = { ...prev };
      if (grouped.size === 0) {
        next[`${selectedConnectionId}::`] = selectedSchemaData;
      }
      for (const [catalog, tables] of grouped) {
        next[`${selectedConnectionId}::${catalog}`] = {
          connectionId: selectedConnectionId,
          tables,
        };
      }
      return next;
    });
  }, [selectedConnectionId, selectedSchemaData, setTreeSchemaCache]);
}

// The workspace controller owns the loading actions. Run root orchestration
// after it is composed so cache ownership does not change loader semantics.
export function useDatabaseTreeRootLoading({
  active,
  connections,
  connectionStates,
  loadConnectionRoot,
}: {
  active: boolean;
  connections: DatabaseConnection[];
  connectionStates: Record<string, DatabaseConnectionSessionState>;
  loadConnectionRoot: (connection: DatabaseConnection) => void;
}) {
  // Read committed loader/cache state without making cache writes re-trigger loads.
  const loadConnectionRootRef = useRef(loadConnectionRoot);
  useLayoutEffect(() => { loadConnectionRootRef.current = loadConnectionRoot; });

  // Eagerly load the first tree level (database list for PostgreSQL/MySQL, file
  // schema for SQLite) for every connected connection. Only the active
  // connection loads through its own queries; without this a second connected
  // connection would sit empty until manually expanded.
  useEffect(() => {
    if (!active) {
      return;
    }
    for (const connection of connections) {
      const status = connectionStates[connection.id]?.status;
      if (canLoadDatabaseSchema(status)) {
        loadConnectionRootRef.current(connection);
      }
    }
  }, [active, connections, connectionStates]);
}
