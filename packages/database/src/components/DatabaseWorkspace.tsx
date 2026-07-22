import { useState } from "react";
import type {
  DatabaseConnection,
  DatabaseSchema,
  DatabaseTableStructure,
} from "@unfour/command-client";
import type {
  DatabaseResultTab,
  DatabaseStructureTab,
  DatabaseWorkspaceTab,
  DatabaseWorkspaceTabId,
  RunSqlOptions,
  SqlHistoryEntry,
  TableEditing,
  TableSegment,
} from "../model/types";
import { ConfirmDialog, SplitPane, Tabs, useI18n, type WorkspaceTab } from "@unfour/ui";
import { QueryResultPanel } from "./QueryResultPanel";
import { SqlEditorTab } from "./SqlEditorTab";
import { TableDataTab } from "./TableDataTab";
import { TableInspector } from "./TableInspector";

export function DatabaseWorkspace({
  activeTabId,
  activeTab,
  catalogOptions,
  connections,
  executePending,
  history,
  onChangeQueryContext,
  onClearHistory,
  onClearSql,
  onCloseTab,
  onPreviewSelectedTable,
  onRefreshSchema,
  onReorderTabs,
  onRun,
  onSelectConnection,
  onSelectResultSet,
  queryCatalog,
  querySchema,
  schemaOptions,
  onSelectHistory,
  onSelectStructureTab,
  onSelectResultTab,
  onSelectTab,
  onSelectTableSegment,
  onShowHistory,
  onSqlChange,
  onStop,
  onTableFilter,
  onTablePageChange,
  onTableSort,
  schema,
  schemaError,
  structure,
  structureError,
  structureLoading,
  tableEditing,
  tabs,
  workspaceId,
}: {
  activeTabId: DatabaseWorkspaceTabId;
  activeTab: DatabaseWorkspaceTab | null;
  catalogOptions: string[];
  connections: DatabaseConnection[];
  executePending: boolean;
  history: SqlHistoryEntry[];
  onChangeQueryContext: (patch: { catalog?: string | null; schema?: string | null }) => void;
  onClearHistory: () => void;
  onClearSql: () => void;
  onCloseTab: (tabId: DatabaseWorkspaceTabId) => void;
  onPreviewSelectedTable: () => void;
  onRefreshSchema: () => void;
  onReorderTabs: (fromIndex: number, toIndex: number) => void;
  onRun: (options?: string | RunSqlOptions) => void;
  onSelectConnection: (connectionId: string) => void;
  onSelectResultSet: (index: number) => void;
  queryCatalog: string | null;
  querySchema: string | null;
  schemaOptions: string[];
  onSelectHistory: (entry: SqlHistoryEntry) => void;
  onSelectStructureTab: (tab: DatabaseStructureTab) => void;
  onSelectResultTab: (tab: DatabaseResultTab) => void;
  onSelectTab: (tabId: DatabaseWorkspaceTabId) => void;
  onSelectTableSegment: (segment: TableSegment) => void;
  onShowHistory: () => void;
  onSqlChange: (sql: string) => void;
  onStop: () => void;
  onTableFilter: (filter: string) => void;
  onTablePageChange: (pageIndex: number, pageSize: number) => void;
  onTableSort: (column: string) => void;
  schema?: DatabaseSchema;
  schemaError: unknown;
  structure?: DatabaseTableStructure | null;
  structureError?: unknown;
  structureLoading?: boolean;
  tableEditing?: TableEditing | null;
  tabs: DatabaseWorkspaceTab[];
  workspaceId: string;
}) {
  const { t } = useI18n();
  const [pendingCloseId, setPendingCloseId] = useState<DatabaseWorkspaceTabId | null>(null);
  const activeQuery = activeTab?.kind === "query" ? activeTab : null;
  const activeTable = activeTab?.kind === "table" ? activeTab : null;

  // Keep-alive: remember the last active query/table so their component trees
  // (especially the Monaco editor inside SqlEditorTab) stay mounted when the
  // other tab type is active.  This prevents the white flash that occurs when
  // React unmounts one branch and mounts a fresh one (e.g. switching from a
  // table tab to a new query tab).
  const [lastQuery, setLastQuery] = useState(activeQuery);
  if (activeQuery && activeQuery !== lastQuery) {
    setLastQuery(activeQuery);
  }
  const [lastTable, setLastTable] = useState(activeTable);
  if (activeTable && activeTable !== lastTable) {
    setLastTable(activeTable);
  }

  // Use the active tab when available; fall back to the last-seen tab so the
  // branch keeps rendering (hidden) with valid props instead of unmounting.
  const renderQuery = activeQuery ?? lastQuery;
  const renderTable = activeTable ?? lastTable;

  const showQuery = Boolean(activeQuery);
  const showTable = Boolean(activeTable);
  const hasTabs = tabs.length > 0;

  const workspaceTabs: WorkspaceTab[] = tabs.map((tab) => ({
    id: tab.id,
    loading: Boolean(tab.loading),
    meta:
      tab.kind === "table" && (tab.pendingChanges?.length ?? 0) > 0 ? (
        <span
          className="h-2 w-2 rounded-full bg-[var(--u-color-warning)]"
          title={t("database.editing.pendingIndicator")}
        />
      ) : null,
    modified: tab.kind === "query" && tab.sql.trim().length > 0,
    title: tab.title,
  }));

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Tabs
        activeId={activeTabId}
        onClose={(tabId) => {
          const tab = tabs.find((candidate) => candidate.id === tabId);
          if (tab?.kind === "table" && (tab.pendingChanges?.length ?? 0) > 0) {
            setPendingCloseId(tabId);
          } else {
            onCloseTab(tabId);
          }
        }}
        onReorder={onReorderTabs}
        onSelect={(tabId) => onSelectTab(tabId as DatabaseWorkspaceTabId)}
        tabs={workspaceTabs}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        {/* Table branch — keep mounted, toggle visibility to avoid remount flash */}
        <div className={showTable ? "flex min-h-0 flex-1 flex-col" : "hidden"}>
          {renderTable && (
            <>
              {renderTable.segment === "data" ? (
                <TableDataTab
                  editing={tableEditing}
                  error={renderTable.error}
                  executePending={executePending}
                  loading={Boolean(renderTable.loading)}
                  onPageChange={onTablePageChange}
                  onRefresh={() =>
                    renderTable.tableView &&
                    onTablePageChange(renderTable.tableView.pageIndex, renderTable.tableView.pageSize)
                  }
                  onSwitchToStructure={() => onSelectTableSegment("structure")}
                  onTableFilter={onTableFilter}
                  onTableSort={onTableSort}
                  result={renderTable.queryResult}
                  table={renderTable.table}
                  tableFilter={renderTable.tableQuery.filter}
                  tableSort={
                    renderTable.tableQuery.orderBy
                      ? {
                          column: renderTable.tableQuery.orderBy,
                          descending: renderTable.tableQuery.orderDescending,
                        }
                      : null
                  }
                  tableView={renderTable.tableView}
                />
              ) : (
                <TableInspector
                  activeTab={renderTable.structureTab}
                  error={renderTable.error ?? structureError ?? schemaError}
                  loading={Boolean(structureLoading)}
                  onPreview={onPreviewSelectedTable}
                  onRefresh={onRefreshSchema}
                  onSelectTab={onSelectStructureTab}
                  onSwitchToData={() => onSelectTableSegment("data")}
                  previewPending={executePending}
                  structure={structure}
                  table={renderTable.table}
                />
              )}
            </>
          )}
        </div>

        {/* Query branch — keep mounted, toggle visibility to avoid remount flash */}
        <div className={showQuery ? "flex min-h-0 flex-1" : "hidden"}>
          {renderQuery && (
            <SplitPane className="min-h-0 flex-1" defaultRatio={62} minPaneSize={220} orientation="vertical" resizable>
              <SqlEditorTab
                catalogOptions={catalogOptions}
                connections={connections}
                executePending={executePending}
                onChangeQueryContext={onChangeQueryContext}
                onClearSql={onClearSql}
                onRun={onRun}
                onSelectConnection={onSelectConnection}
                onShowHistory={onShowHistory}
                onSqlChange={onSqlChange}
                onStop={onStop}
                pendingConfirmation={renderQuery.pendingConfirmation}
                queryCatalog={queryCatalog}
                querySchema={querySchema}
                schema={schema}
                schemaOptions={schemaOptions}
                selectedConnectionId={renderQuery.connectionId}
                sql={renderQuery.sql}
                workspaceId={workspaceId}
              />
              <QueryResultPanel
                activeResultIndex={renderQuery.activeResultIndex}
                activeTab={renderQuery.resultTab}
                error={renderQuery.error}
                history={history}
                isPending={executePending}
                onClearHistory={onClearHistory}
                onSelectHistory={onSelectHistory}
                onSelectResultSet={onSelectResultSet}
                onSelectTab={onSelectResultTab}
                pendingConfirmation={renderQuery.pendingConfirmation}
                result={renderQuery.result}
                results={renderQuery.results}
              />
            </SplitPane>
          )}
        </div>

        {/* Empty state — shown only when no tabs exist at all */}
        {!hasTabs && <EmptyWorkspace />}
      </div>
      <ConfirmDialog
        confirmLabel={t("database.editing.discard")}
        description={t("database.editing.discardBody")}
        onConfirm={() => {
          if (pendingCloseId) onCloseTab(pendingCloseId);
          setPendingCloseId(null);
        }}
        onOpenChange={(open) => !open && setPendingCloseId(null)}
        open={pendingCloseId !== null}
        pending={Boolean(tableEditing?.pending)}
        title={t("database.editing.discardTitle")}
      />
    </div>
  );
}

function EmptyWorkspace() {
  const { t } = useI18n();
  return <div className="p-3 text-[12px] text-[var(--u-color-text-soft)]">{t("database.editor.empty")}</div>;
}

