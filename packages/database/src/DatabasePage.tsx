import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { DatabaseTable } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
  ConfirmDialog,
  useI18n,
} from "@unfour/ui";
import { DatabaseTestResultDialog } from "./components/DatabaseTestResultDialog";
import { DatabaseModuleToolbar } from "./components/DatabaseModuleToolbar";
import { DatabaseConnectionErrorBanner } from "./components/DatabaseConnectionErrorBanner";
import { DatabaseConnectionDialog } from "./components/DatabaseConnectionDialog";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { useDatabaseConnections } from "./hooks/useDatabaseConnections";
import { useDatabaseConnectionMutations } from "./hooks/useDatabaseConnectionMutations";
import { useDatabaseWorkspaceController } from "./hooks/useDatabaseWorkspaceController";
import { useDatabaseTabs } from "./hooks/useDatabaseTabs";
import { useQueryHistory } from "./hooks/useQueryHistory";
import { useSavedSql } from "./hooks/useSavedSql";
import { useTableStructure } from "./hooks/useTableStructure";
import { useDatabaseQueryContext, useDatabaseTabSelection } from "./hooks/useDatabaseTabSynchronization";
import { EMPTY_CONNECTION_STATES, useDatabaseConnectionStore } from "./model/database-connection-state";
import type {
  DatabaseConnectionSessionState,
  SqlHistoryEntry,
} from "./model/types";
import { useDatabaseConnectionForm } from "./hooks/useDatabaseConnectionForm";
import type { DatabasePageProps } from "./model/database-page";
import { useDatabaseActiveContext, useDatabaseActiveTableEditing, useDatabaseContextOptions } from "./hooks/useDatabaseActiveContext";
import { useDatabaseShellIntegration } from "./hooks/useDatabaseShellIntegration";
import { useDatabaseTreeController, useDatabaseTreeRootLoading, useDatabaseTreeSynchronization } from "./hooks/useDatabaseTreeController";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;
const MAX_HISTORY_ENTRIES = 25;

export function DatabasePage({
  active = true,
  onShellSidebarChange,
  onShellStatusBarChange,
  statusBarRightAccessory,
  workspaceName,
  workspaceId,
}: DatabasePageProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    selectedDatabaseConnectionId: selectedConnectionId,
    setSelectedDatabaseConnection,
  } = useWorkspaceStore();
  const databaseTabs = useDatabaseTabs({
    formatQueryTitle: (index) => t("database.editor.queryTitle", { index }),
    workspaceId,
  });
  const connectionStates = useDatabaseConnectionStore(
    (state) => state.byWorkspace[workspaceId] ?? EMPTY_CONNECTION_STATES,
  );
  const setConnectionStateAction = useDatabaseConnectionStore((state) => state.setConnectionState);
  const pruneConnectionsAction = useDatabaseConnectionStore((state) => state.pruneConnections);
  const removeConnectionAction = useDatabaseConnectionStore((state) => state.removeConnection);
  // Bind the workspace id so the existing call sites keep their original
  // `(connectionId, patch)` / `(connectionId)` signatures.
  const setConnectionState = useCallback(
    (connectionId: string, patch: Partial<DatabaseConnectionSessionState>) =>
      setConnectionStateAction(workspaceId, connectionId, patch),
    [setConnectionStateAction, workspaceId],
  );
  const removeConnection = (connectionId: string) => removeConnectionAction(workspaceId, connectionId);
  const [queryHistory, setQueryHistory] = useState<SqlHistoryEntry[]>([]);
  const [selectedTable, setSelectedTable] = useState<DatabaseTable | null>(null);
  const connectionsQuery = useDatabaseConnections(workspaceId, { active });
  const queryHistoryQuery = useQueryHistory(workspaceId, MAX_HISTORY_ENTRIES, { active });
  const savedSqlQuery = useSavedSql(workspaceId, { active });
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const activeTab = databaseTabs.activeTab;
  const context = useDatabaseActiveContext({
    active,
    activeTab,
    connections,
    connectionStates,
    selectedConnectionId,
    selectedTable,
  });
  const { activeQueryTab, activeTableTab, selectedConnection, selectedConnectionStatus } = context;
  const {
    editorOpen,
    setEditorOpen,
    testResult,
    setTestResult,
    password,
    setPassword,
    form,
    setForm,
    hydrateFormFromConnection,
  } = useDatabaseConnectionForm(workspaceId, selectedConnectionId, selectedConnection);
  const tree = useDatabaseTreeController({
    schemaEnabled: context.schemaEnabled,
    selectedConnection,
    selectedConnectionId,
    workspaceId,
  });
  const { catalogOptions, schemaOptions } = useDatabaseContextOptions({
    activeQueryConnection: context.activeQueryConnection,
    activeQueryTab,
    catalogNames: tree.catalogsQuery.data,
    treeModel: tree.treeModel,
  });
  const structureQuery = useTableStructure({
    connectionId: activeTableTab?.connectionId ?? null,
    enabled: context.structureEnabled,
    table: activeTableTab?.table ?? null,
    workspaceId,
  });
  useDatabaseTabSelection(activeTab, setSelectedDatabaseConnection, setSelectedTable);

  useEffect(() => {
    if (selectedConnectionId && !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedDatabaseConnection(null);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- clearing derived state when parent selection is removed
      setSelectedTable(null);
    }

    // Drop state for connections that no longer exist. Read from the store
    // directly (not a hook selector) so this effect does not re-run on every
    // connection-state change and loop.
    const liveIds = new Set(connections.map((connection) => connection.id));
    const current = useDatabaseConnectionStore.getState().byWorkspace[workspaceId] ?? {};
    if (Object.keys(current).some((id) => !liveIds.has(id))) {
      pruneConnectionsAction(workspaceId, liveIds);
    }
  }, [connections, selectedConnectionId, setSelectedDatabaseConnection, workspaceId, pruneConnectionsAction]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- syncing persisted query history into local optimistic UI state
    setQueryHistory(queryHistoryQuery.entries.slice(0, MAX_HISTORY_ENTRIES));
  }, [queryHistoryQuery.entries]);

  useDatabaseTreeSynchronization(tree, {
    schemaEnabled: context.schemaEnabled,
    selectedConnectionId,
    setConnectionState,
  });
  useDatabaseQueryContext(activeTab, tree.treeModel, context.activeQueryConnection?.driver === "sqlite" ? null : context.activeQueryConnection?.database, databaseTabs.updateQueryTab);

  const {
    deleteConfirm,
    deleteMutation,
    duplicateMutation,
    saveMutation,
    setDeleteConfirm,
    testInputMutation,
    testMutation,
  } = useDatabaseConnectionMutations({
    databaseTabs,
    hydrateFormFromConnection,
    queryClient,
    removeConnection,
    selectedConnectionId,
    setConnectionState,
    setEditorOpen,
    setSelectedDatabaseConnection,
    setSelectedTable,
    setTestResult,
    t,
    workspaceId,
  });

  const {
    applyTableFilter,
    applyPendingTableChanges,
    applyTableSort,
    browseMutation,
    browseTablePage,
    canTest,
    changeQueryContext,
    clearQueryHistory,
    clearSql,
    connectConnection,
    deleteSavedSql,
    designTable,
    disconnectConnection,
    handleEditConnection,
    handleNewConnection,
    handleSelectResultTab,
    handleSelectStructureTab,
    handleSelectTableSegment,
    handleTablePageChange,
    loadCatalogSchema,
    loadConnectionRoot,
    loadHistoryEntry,
    loadSqlIntoEditor,
    markActiveSqlSaved,
    openSavedSql,
    previewSelectedTable,
    refreshActiveSchema,
    refreshConnectionSchema,
    refreshConnectionsAndSchema,
    rowMutation,
    runSql,
    selectConnection,
    selectDatabaseTab,
    selectQueryConnection,
    selectQueryResult,
    selectTable,
    showQueryHistory,
    sqlRunning,
    startNewQuery,
    stopQuery,
    submitConnection,
    testConnectionInput,
    updateActiveSql,
    updateForm,
  } = useDatabaseWorkspaceController({
    activeQueryTab,
    activeTableTab,
    catalogNamesByConn: tree.catalogNamesByConn,
    connectionStates,
    connections,
    databaseTabs,
    form,
    hydrateFormFromConnection,
    maxHistoryEntries: MAX_HISTORY_ENTRIES,
    password,
    queryClient,
    queryHistoryQuery,
    saveMutation,
    savedSqlQuery,
    selectedConnection,
    selectedConnectionId,
    selectedConnectionStatus,
    selectedTable,
    setCatalogNamesByConn: tree.setCatalogNamesByConn,
    setConnectionState,
    setEditorOpen,
    setForm,
    setPassword,
    setQueryHistory,
    setSelectedDatabaseConnection,
    setSelectedTable,
    setTestResult,
    setTreeErrors: tree.setTreeErrors,
    setTreeLoadingKeys: tree.setTreeLoadingKeys,
    setTreeSchemaCache: tree.setTreeSchemaCache,
    t,
    testInputMutation,
    testMutation,
    treeLoadingKeys: tree.treeLoadingKeys,
    treeSchemaCache: tree.treeSchemaCache,
    workspaceId,
  });

  useDatabaseTreeRootLoading({ active, connections, connectionStates, loadConnectionRoot });
  const executePending = sqlRunning || browseMutation.isPending || rowMutation.isPending;
  useDatabaseShellIntegration({
    active,
    catalogNamesByConn: tree.catalogNamesByConn,
    connections,
    connectionStates,
    executePending,
    onShellSidebarChange,
    onShellStatusBarChange,
    savedSql: savedSqlQuery.saved,
    selectedConnectionId,
    selectedTableId: context.selectedTableId,
    sidebarActions: {
      connect: connectConnection,
      delete: setDeleteConfirm,
      deleteSavedSql,
      designTable,
      disconnect: disconnectConnection,
      duplicate: (connection) => duplicateMutation.mutate(connection),
      edit: handleEditConnection,
      newConnection: handleNewConnection,
      newQuery: (connection, catalog) => startNewQuery(connection?.id, catalog),
      openSavedSql,
      previewTable: (connectionId, table) =>
        browseTablePage(connectionId, table, 0, DEFAULT_PREVIEW_PAGE_SIZE),
      refresh: refreshConnectionsAndSchema,
      refreshSchema: refreshConnectionSchema,
      selectConnection: (connection) => selectConnection(connection.id),
      selectTable,
      toggleCatalog: loadCatalogSchema,
      toggleConnection: loadConnectionRoot,
      useSql: loadSqlIntoEditor,
    },
    statusBarRightAccessory,
    toolbarConnection: context.toolbarConnection,
    toolbarSession: context.toolbarSession,
    treeErrors: tree.treeErrors,
    treeLoadingKeys: tree.treeLoadingKeys,
    treeSchemaCache: tree.treeSchemaCache,
    workspaceId,
    workspaceName,
  });
  const tableEditing = useDatabaseActiveTableEditing({
    applyPendingChanges: applyPendingTableChanges,
    connection: context.activeTableConnection,
    connected: context.activeTableStatus === "connected",
    mutationPending: rowMutation.isPending,
    tab: activeTableTab,
    updateTableTab: databaseTabs.updateTableTab,
  });

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <DatabaseModuleToolbar
        connectionStatus={context.toolbarConnectionStatus}
        onNewQuery={startNewQuery}
        selectedConnectionName={context.toolbarConnection?.name ?? null}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        {selectedConnection && selectedConnectionStatus === "failed" ? (
          <DatabaseConnectionErrorBanner
            connectionName={selectedConnection.name}
            message={context.selectedSession?.message}
            onEdit={() => handleEditConnection(selectedConnection)}
            onRetry={() => connectConnection(selectedConnection)}
          />
        ) : null}
        <DatabaseWorkspace
          active={active}
          activeTab={activeTab}
          activeTabId={databaseTabs.activeTabId}
          connections={connections}
          executePending={executePending}
          history={queryHistory}
          catalogOptions={catalogOptions}
          onChangeQueryContext={changeQueryContext}
          onClearSql={clearSql}
          onCloseTab={databaseTabs.closeTab}
          onClearHistory={clearQueryHistory}
          onPreviewSelectedTable={previewSelectedTable}
          onRefreshSchema={refreshActiveSchema}
          onReorderTabs={databaseTabs.reorderTabs}
          onRun={runSql}
          onSelectConnection={(connectionId) => selectQueryConnection(connectionId || null)}
          onSelectResultSet={selectQueryResult}
          queryCatalog={activeQueryTab?.catalog ?? null}
          querySchema={activeQueryTab?.schema ?? null}
          schemaOptions={schemaOptions}
          onSelectHistory={loadHistoryEntry}
          onSelectResultTab={handleSelectResultTab}
          onSelectStructureTab={handleSelectStructureTab}
          onSelectTab={selectDatabaseTab}
          onSelectTableSegment={handleSelectTableSegment}
          onShowHistory={showQueryHistory}
          onOpenSavedSql={openSavedSql}
          onSqlChange={updateActiveSql}
          onSqlSaved={markActiveSqlSaved}
          onStop={stopQuery}
          onTableFilter={applyTableFilter}
          onTablePageChange={handleTablePageChange}
          onTableSort={applyTableSort}
          schema={tree.visibleSchema}
          schemaError={tree.schemaQuery.error}
          structure={structureQuery.data}
          structureError={structureQuery.error}
          structureLoading={context.structureEnabled && structureQuery.isFetching}
          tableEditing={tableEditing}
          tabs={databaseTabs.tabs}
          workspaceId={workspaceId}
        />
      </div>
      <DatabaseConnectionDialog
        canTest={canTest}
        error={saveMutation.error}
        form={form}
        key={workspaceId}
        onOpenChange={setEditorOpen}
        onPasswordChange={setPassword}
        onSubmit={submitConnection}
        onTest={testConnectionInput}
        onUpdate={updateForm}
        open={editorOpen}
        password={password}
        savePending={saveMutation.isPending}
        testPending={testInputMutation.isPending}
      >
        <DatabaseTestResultDialog
          onOpenChange={(open) => !open && setTestResult(null)}
          result={testResult}
        />
      </DatabaseConnectionDialog>
      <ConfirmDialog
        confirmLabel={t("common.actions.delete")}
        description={
          deleteConfirm ? t("database.tree.deleteBody", { name: deleteConfirm.name }) : ""
        }
        onConfirm={() => deleteConfirm && deleteMutation.mutate(deleteConfirm.id)}
        onOpenChange={(open) => !open && setDeleteConfirm(null)}
        open={deleteConfirm !== null}
        pending={deleteMutation.isPending}
        title={t("database.tree.deleteTitle")}
      />
    </div>
  );
}
