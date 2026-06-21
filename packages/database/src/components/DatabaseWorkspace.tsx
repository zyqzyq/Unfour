import type { DatabaseConnection, DatabaseQueryResult, DatabaseSchema, DatabaseTable } from "@unfour/command-client";
import type { DatabaseResultTab, DatabaseTableViewState, SqlHistoryEntry } from "../model/types";
import { Tabs, useI18n } from "@unfour/ui";
import { QueryResultPanel } from "./QueryResultPanel";
import { SqlEditorTab } from "./SqlEditorTab";
import { TableDataTab } from "./TableDataTab";
import { TableInspector } from "./TableInspector";

export function DatabaseWorkspace({
  activeResultTab,
  activeStructureTab,
  activeTabId,
  connections,
  error,
  executePending,
  history,
  onClearHistory,
  onClearSql,
  onPreviewSelectedTable,
  onRefreshSchema,
  onRun,
  onSelectConnection,
  onSelectHistory,
  onSelectStructureTab,
  onSelectResultTab,
  onSelectTab,
  onSqlChange,
  onStop,
  onTablePageChange,
  pendingConfirmation,
  queryResult,
  schema,
  schemaError,
  schemaLoading,
  selectedConnectionId,
  selectedTable,
  sql,
  tableView,
}: {
  activeResultTab: DatabaseResultTab;
  activeStructureTab: "columns" | "indexes" | "constraints" | "properties" | "ddl";
  activeTabId: string;
  connections: DatabaseConnection[];
  error: unknown;
  executePending: boolean;
  history: SqlHistoryEntry[];
  onClearHistory: () => void;
  onClearSql: () => void;
  onPreviewSelectedTable: () => void;
  onRefreshSchema: () => void;
  onRun: (selectedSql?: string) => void;
  onSelectConnection: (connectionId: string) => void;
  onSelectHistory: (entry: SqlHistoryEntry) => void;
  onSelectStructureTab: (tab: "columns" | "indexes" | "constraints" | "properties" | "ddl") => void;
  onSelectResultTab: (tab: DatabaseResultTab) => void;
  onSelectTab: (tabId: string) => void;
  onSqlChange: (sql: string) => void;
  onStop: () => void;
  onTablePageChange: (pageIndex: number, pageSize: number) => void;
  pendingConfirmation: boolean;
  queryResult: DatabaseQueryResult | null;
  schema?: DatabaseSchema;
  schemaError: unknown;
  schemaLoading: boolean;
  selectedConnectionId: string | null;
  selectedTable: DatabaseTable | null;
  sql: string;
  tableView: DatabaseTableViewState | null;
}) {
  const { t } = useI18n();

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Tabs
        activeId={activeTabId}
        onSelect={onSelectTab}
        tabs={[
          {
            id: "sql-editor",
            loading: executePending,
            modified: sql.trim().length > 0,
            title: t("database.editor.sqlEditor"),
          },
          {
            id: "table-structure",
            loading: schemaLoading,
            meta: selectedTable ? <span className="text-[11px] text-[var(--u-color-text-soft)]">{selectedTable.name}</span> : null,
            title: t("database.editor.tableStructure"),
          },
          {
            id: "table-data",
            loading: executePending,
            meta: tableView ? <span className="text-[11px] text-[var(--u-color-text-soft)]">{tableView.tableName}</span> : null,
            title: t("database.editor.tableData"),
          },
        ]}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        {activeTabId === "table-data" ? (
          <TableDataTab
            executePending={executePending}
            onPageChange={onTablePageChange}
            onRefresh={() => tableView && onTablePageChange(tableView.pageIndex, tableView.pageSize)}
            result={queryResult}
            tableView={tableView}
          />
        ) : activeTabId === "table-structure" ? (
          <TableInspector
            activeTab={activeStructureTab}
            error={schemaError}
            loading={schemaLoading}
            onPreview={onPreviewSelectedTable}
            onRefresh={onRefreshSchema}
            onSelectTab={onSelectStructureTab}
            previewPending={executePending}
            table={selectedTable}
          />
        ) : (
          <SqlEditorTab
            connections={connections}
            executePending={executePending}
            onClearSql={onClearSql}
            onRun={onRun}
            onSelectConnection={onSelectConnection}
            onSqlChange={onSqlChange}
            onStop={onStop}
            pendingConfirmation={pendingConfirmation}
            schema={schema}
            selectedConnectionId={selectedConnectionId}
            sql={sql}
          />
        )}
        <QueryResultPanel
          activeTab={activeResultTab}
          error={error}
          history={history}
          isPending={executePending}
          onClearHistory={onClearHistory}
          onSelectHistory={onSelectHistory}
          onSelectTab={onSelectResultTab}
          pendingConfirmation={pendingConfirmation}
          result={queryResult}
        />
      </div>
    </div>
  );
}
