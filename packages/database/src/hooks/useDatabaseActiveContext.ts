import { useMemo } from "react";
import type { DatabaseConnection, DatabaseTable } from "@unfour/command-client";
import { canLoadDatabaseSchema } from "../model/database-connection-state";
import { databaseTableTreeId, type DatabaseTreeModel } from "../model/database-tree";
import { createTableEditing } from "../model/table-editing";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseQueryWorkspaceTab,
  DatabaseWorkspaceTab,
} from "../model/types";

export function useDatabaseActiveContext({
  active,
  activeTab,
  connections,
  connectionStates,
  selectedConnectionId,
  selectedTable,
}: {
  active: boolean;
  activeTab: DatabaseWorkspaceTab | null;
  connections: DatabaseConnection[];
  connectionStates: Record<string, DatabaseConnectionSessionState>;
  selectedConnectionId: string | null;
  selectedTable: DatabaseTable | null;
}) {
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  const selectedSession = selectedConnectionId ? connectionStates[selectedConnectionId] : undefined;
  const selectedConnectionStatus: DatabaseConnectionStatus = selectedSession?.status ?? "disconnected";
  const activeQueryTab = activeTab?.kind === "query" ? activeTab : null;
  const activeTableTab = activeTab?.kind === "table" ? activeTab : null;
  const activeQueryConnection = connections.find(
    (connection) => connection.id === activeQueryTab?.connectionId,
  );
  const toolbarConnectionId = activeQueryTab
    ? activeQueryTab.connectionId
    : activeTableTab?.connectionId ?? selectedConnectionId;
  const toolbarConnection = connections.find((item) => item.id === toolbarConnectionId) ?? null;
  const toolbarSession = toolbarConnectionId ? connectionStates[toolbarConnectionId] : undefined;
  const toolbarConnectionStatus: DatabaseConnectionStatus = toolbarSession?.status ?? "disconnected";
  const activeTableConnection = activeTableTab
    ? connections.find((connection) => connection.id === activeTableTab.connectionId) ?? null
    : null;
  const activeTableStatus = activeTableTab
    ? connectionStates[activeTableTab.connectionId]?.status ?? "disconnected"
    : "disconnected";

  return {
    activeQueryConnection,
    activeQueryTab,
    activeTableConnection,
    activeTableStatus,
    activeTableTab,
    schemaEnabled: Boolean(active && selectedConnection && canLoadDatabaseSchema(selectedConnectionStatus)),
    selectedConnection,
    selectedConnectionStatus,
    selectedSession,
    selectedTableId: selectedConnectionId && selectedTable
      ? databaseTableTreeId(selectedConnectionId, selectedTable)
      : null,
    structureEnabled: Boolean(
      active && activeTableTab && activeTableTab.segment === "structure" &&
        canLoadDatabaseSchema(connectionStates[activeTableTab.connectionId]?.status),
    ),
    toolbarConnection,
    toolbarConnectionStatus,
    toolbarSession,
  };
}

export function useDatabaseContextOptions({
  activeQueryConnection,
  activeQueryTab,
  catalogNames,
  treeModel,
}: {
  activeQueryConnection: DatabaseConnection | undefined;
  activeQueryTab: DatabaseQueryWorkspaceTab | null;
  catalogNames: string[] | undefined;
  treeModel: DatabaseTreeModel | null;
}) {
  // Preserve the selected schema's choices even when the query targets another
  // catalog. Lazy tree cache entries do not drive query context normalization.
  const catalogOptions = useMemo(() => {
    const merged = new Set<string>();
    for (const name of catalogNames ?? []) {
      if (name) merged.add(name);
    }
    for (const catalog of treeModel?.catalogs ?? []) {
      if (catalog.key) merged.add(catalog.key);
    }
    return [...merged];
  }, [catalogNames, treeModel]);
  const schemaOptions = useMemo(() => {
    if (!treeModel) return [];
    const activeCatalogKey = activeQueryTab?.catalog ?? activeQueryConnection?.database ?? null;
    const activeCatalog = treeModel.catalogs.find((catalog) => catalog.key === activeCatalogKey);
    if (!activeCatalog?.hasSchemaLevel) return [];
    return activeCatalog.schemas.map((schema) => schema.key).filter((key) => key !== "");
  }, [activeQueryConnection?.database, activeQueryTab?.catalog, treeModel]);

  return { catalogOptions, schemaOptions };
}

export function useDatabaseActiveTableEditing({
  applyPendingChanges,
  connection,
  connected,
  mutationPending,
  tab,
  updateTableTab,
}: Parameters<typeof createTableEditing>[0]) {
  return useMemo(
    () => createTableEditing({ applyPendingChanges, connection, connected, mutationPending, tab, updateTableTab }),
    [applyPendingChanges, connection, connected, mutationPending, tab, updateTableTab],
  );
}
