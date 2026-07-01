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
  SqlHistoryEntry,
  TableEditing,
  TableSegment,
} from "../model/types";
import { SplitPane, Tabs, useI18n, type WorkspaceTab } from "@unfour/ui";
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
  onRun: (selectedSql?: string) => void;
  onSelectConnection: (connectionId: string) => void;
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
  const activeQuery = activeTab?.kind === "query" ? activeTab : null;
  const activeTable = activeTab?.kind === "table" ? activeTab : null;
  const workspaceTabs: WorkspaceTab[] = tabs.map((tab) => ({
    id: tab.id,
    loading: Boolean(tab.loading),
    modified: tab.kind === "query" && tab.sql.trim().length > 0,
    title: tab.title,
  }));

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Tabs
        activeId={activeTabId}
        onClose={onCloseTab}
        onReorder={onReorderTabs}
        onSelect={(tabId) => onSelectTab(tabId as DatabaseWorkspaceTabId)}
        tabs={workspaceTabs}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        {activeTable ? (
          <>
            {activeTable.segment === "data" ? (
              <TableDataTab
                editing={tableEditing}
                error={activeTable.error}
                executePending={executePending}
                onPageChange={onTablePageChange}
                onRefresh={() =>
                  activeTable.tableView &&
                  onTablePageChange(activeTable.tableView.pageIndex, activeTable.tableView.pageSize)
                }
                onSwitchToStructure={() => onSelectTableSegment("structure")}
                onTableFilter={onTableFilter}
                onTableSort={onTableSort}
                result={activeTable.queryResult}
                tableFilter={activeTable.tableQuery.filter}
                tableSort={
                  activeTable.tableQuery.orderBy
                    ? {
                        column: activeTable.tableQuery.orderBy,
                        descending: activeTable.tableQuery.orderDescending,
                      }
                    : null
                }
                tableView={activeTable.tableView}
              />
            ) : (
              <TableInspector
                activeTab={activeTable.structureTab}
                error={activeTable.error ?? structureError ?? schemaError}
                loading={Boolean(structureLoading)}
                onPreview={onPreviewSelectedTable}
                onRefresh={onRefreshSchema}
                onSelectTab={onSelectStructureTab}
                onSwitchToData={() => onSelectTableSegment("data")}
                previewPending={executePending}
                structure={structure}
                table={activeTable.table}
              />
            )}
          </>
        ) : activeQuery ? (
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
              pendingConfirmation={activeQuery.pendingConfirmation}
              queryCatalog={queryCatalog}
              querySchema={querySchema}
              schema={schema}
              schemaOptions={schemaOptions}
              selectedConnectionId={activeQuery.connectionId}
              sql={activeQuery.sql}
              workspaceId={workspaceId}
            />
            <QueryResultPanel
              activeTab={activeQuery.resultTab}
              error={activeQuery.error}
              history={history}
              isPending={executePending}
              onClearHistory={onClearHistory}
              onSelectHistory={onSelectHistory}
              onSelectTab={onSelectResultTab}
              pendingConfirmation={activeQuery.pendingConfirmation}
              result={activeQuery.result}
            />
          </SplitPane>
        ) : (
          <EmptyWorkspace />
        )}
      </div>
    </div>
  );
}

function EmptyWorkspace() {
  const { t } = useI18n();
  return <div className="p-3 text-[12px] text-[var(--u-color-text-soft)]">{t("database.editor.empty")}</div>;
}

