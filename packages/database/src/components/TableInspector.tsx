import { Play, RefreshCw } from "lucide-react";
import type { DatabaseTable, DatabaseTableStructure } from "@unfour/command-client";
import {
  Button,
  EmptyState,
  ErrorState,
  IconButton,
  LoadingState,
  SplitPane,
  StatusBadge,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

// Detail sub-views shown in the lower pane; the columns grid is always pinned
// to the top pane, matching the TablePlus/DataGrip structure layout.
type StructureTab = "ddl" | "indexes" | "constraints" | "properties";

export function TableInspector({
  activeTab,
  error,
  loading = false,
  onPreview,
  onRefresh,
  onSelectTab,
  previewPending = false,
  structure,
  table,
}: {
  activeTab: StructureTab;
  error?: unknown;
  loading?: boolean;
  onPreview?: () => void;
  onRefresh?: () => void;
  onSelectTab: (tab: StructureTab) => void;
  previewPending?: boolean;
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable | null;
}) {
  const { t } = useI18n();

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar className="h-8">
        <ToolbarGroup className="min-w-0">
          <span className="truncate text-[12px] font-semibold text-[var(--u-color-text)]">
            {table ? qualifiedTableName(table) : t("database.structure.title")}
          </span>
          {table ? <StatusBadge>{table.kind}</StatusBadge> : null}
        </ToolbarGroup>
        <ToolbarGroup>
          <IconButton disabled={loading} label={t("database.structure.refresh")} onClick={onRefresh}>
            <RefreshCw size={13} />
          </IconButton>
          <Button disabled={!table || previewPending} onClick={onPreview} size="sm" type="button">
            <Play size={13} />
            {t("database.structure.preview")}
          </Button>
        </ToolbarGroup>
      </Toolbar>
      {renderStructureBody({ activeTab, error, loading, onSelectTab, structure, table, t })}
    </div>
  );
}

function renderStructureBody({
  activeTab,
  error,
  loading,
  onSelectTab,
  structure,
  table,
  t,
}: {
  activeTab: StructureTab;
  error?: unknown;
  loading: boolean;
  onSelectTab: (tab: StructureTab) => void;
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable | null;
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (error) {
    return (
      <ErrorState className="m-2 min-h-0 flex-1">
        <DatabaseErrorDetails error={error} />
      </ErrorState>
    );
  }

  if (loading) {
    return <LoadingState className="m-2 min-h-0 flex-1">{t("database.structure.loading")}</LoadingState>;
  }

  if (!table) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.structure.selectTable")}</EmptyState>;
  }

  return (
    <SplitPane className="min-h-0 flex-1" defaultRatio={62} minPaneSize={120} orientation="vertical" resizable>
      <ColumnsGrid structure={structure} table={table} t={t} />
      <DetailPane activeTab={activeTab} onSelectTab={onSelectTab} structure={structure} table={table} t={t} />
    </SplitPane>
  );
}

function ColumnsGrid({
  structure,
  table,
  t,
}: {
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable;
  t: ReturnType<typeof useI18n>["t"];
}) {
  // Prefer the on-demand structure (with defaults) but fall back to the
  // lightweight schema columns while it loads.
  const columns = structure?.columns ?? table.columns;

  if (!columns.length) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.structure.emptyColumns")}</EmptyState>;
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <table className="w-full table-fixed text-left text-[12px]">
        <colgroup>
          <col className="w-[30%]" />
          <col className="w-[26%]" />
          <col className="w-[14%]" />
          <col className="w-[18%]" />
          <col className="w-[12%]" />
        </colgroup>
        <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
          <tr>
            <ColumnHeader>{t("database.structure.colName")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.colType")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.colNullable")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.colDefault")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.colKey")}</ColumnHeader>
          </tr>
        </thead>
        <tbody>
          {columns.map((column) => (
            <tr
              className="border-b border-[color:color-mix(in_srgb,var(--u-color-border)_62%,transparent)] hover:bg-[var(--u-color-surface-hover)]"
              key={column.name}
            >
              <Cell strong>{column.name}</Cell>
              <Cell>{column.dataType || "ANY"}</Cell>
              <Cell>
                <StatusBadge tone={column.nullable ? "neutral" : "success"}>
                  {column.nullable ? t("database.structure.yes") : t("database.structure.no")}
                </StatusBadge>
              </Cell>
              {column.defaultValue != null ? <Cell>{column.defaultValue}</Cell> : <Cell muted>—</Cell>}
              <Cell>{column.primaryKey ? <StatusBadge tone="success">PK</StatusBadge> : ""}</Cell>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function DetailPane({
  activeTab,
  onSelectTab,
  structure,
  table,
  t,
}: {
  activeTab: StructureTab;
  onSelectTab: (tab: StructureTab) => void;
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable;
  t: ReturnType<typeof useI18n>["t"];
}) {
  const tabs: Array<{ id: StructureTab; label: string }> = [
    { id: "ddl", label: t("database.structure.tabDdl") },
    { id: "indexes", label: t("database.structure.tabIndexes") },
    { id: "constraints", label: t("database.structure.tabConstraints") },
    { id: "properties", label: t("database.structure.tabProperties") },
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col border-t border-[var(--u-color-border)]">
      <div className="flex h-7 shrink-0 items-center gap-1 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-1">
        {tabs.map((tab) => {
          const active = tab.id === activeTab;
          return (
            <button
              className={`h-[22px] rounded-[5px] px-2.5 text-[12px] font-medium ${
                active
                  ? "bg-[var(--u-color-surface)] text-[var(--u-color-text)] shadow-[0_1px_2px_rgba(0,0,0,0.08)]"
                  : "text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
              }`}
              key={tab.id}
              onClick={() => onSelectTab(tab.id)}
              type="button"
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      <div className="min-h-0 flex-1 overflow-auto">{renderDetail({ activeTab, structure, table, t })}</div>
    </div>
  );
}

function renderDetail({
  activeTab,
  structure,
  table,
  t,
}: {
  activeTab: StructureTab;
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable;
  t: ReturnType<typeof useI18n>["t"];
}) {
  const columns = structure?.columns ?? table.columns;

  if (activeTab === "indexes") {
    const indexes = structure?.indexes ?? [];
    if (!indexes.length) {
      return <EmptyState className="m-2 min-h-0 flex-1">{t("database.structure.emptyIndexes")}</EmptyState>;
    }
    return (
      <table className="w-full table-fixed text-left text-[12px]">
        <colgroup>
          <col className="w-[34%]" />
          <col className="w-[40%]" />
          <col className="w-[13%]" />
          <col className="w-[13%]" />
        </colgroup>
        <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
          <tr>
            <ColumnHeader>{t("database.structure.colName")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.idxColumns")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.idxUnique")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.idxPrimary")}</ColumnHeader>
          </tr>
        </thead>
        <tbody>
          {indexes.map((index) => (
            <tr
              className="border-b border-[color:color-mix(in_srgb,var(--u-color-border)_62%,transparent)] hover:bg-[var(--u-color-surface-hover)]"
              key={index.name}
            >
              <Cell strong>{index.name}</Cell>
              <Cell>{index.columns.join(", ")}</Cell>
              <Cell>{index.unique ? <StatusBadge tone="success">{t("database.structure.yes")}</StatusBadge> : ""}</Cell>
              <Cell>{index.primary ? <StatusBadge tone="success">PK</StatusBadge> : ""}</Cell>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  if (activeTab === "constraints") {
    const foreignKeys = structure?.foreignKeys ?? [];
    if (!foreignKeys.length) {
      return <EmptyState className="m-2 min-h-0 flex-1">{t("database.structure.emptyConstraints")}</EmptyState>;
    }
    return (
      <table className="w-full table-fixed text-left text-[12px]">
        <colgroup>
          <col className="w-[28%]" />
          <col className="w-[32%]" />
          <col className="w-[40%]" />
        </colgroup>
        <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
          <tr>
            <ColumnHeader>{t("database.structure.colName")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.fkColumns")}</ColumnHeader>
            <ColumnHeader>{t("database.structure.fkReferences")}</ColumnHeader>
          </tr>
        </thead>
        <tbody>
          {foreignKeys.map((fk) => (
            <tr
              className="border-b border-[color:color-mix(in_srgb,var(--u-color-border)_62%,transparent)] hover:bg-[var(--u-color-surface-hover)]"
              key={fk.name}
            >
              <Cell strong>{fk.name}</Cell>
              <Cell>{fk.columns.join(", ")}</Cell>
              <Cell>
                {fk.referencedTable} ({fk.referencedColumns.join(", ")})
              </Cell>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  if (activeTab === "properties") {
    return (
      <div className="p-2 text-[12px]">
        <Property label={t("database.structure.propSchema")} value={table.schema ?? "default"} />
        <Property label={t("database.structure.propName")} value={table.name} />
        <Property label={t("database.structure.propKind")} value={table.kind} />
        <Property label={t("database.structure.propColumns")} value={String(columns.length)} />
        <Property label={t("database.structure.propIndexes")} value={String(structure?.indexes.length ?? 0)} />
        <Property label={t("database.structure.propForeignKeys")} value={String(structure?.foreignKeys.length ?? 0)} />
      </div>
    );
  }

  // ddl
  if (!structure?.ddl) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.structure.emptyDdl")}</EmptyState>;
  }
  return (
    <pre className="m-2 whitespace-pre-wrap break-words rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2 font-mono text-[12px] text-[var(--u-color-text)]">
      {structure.ddl}
    </pre>
  );
}

function ColumnHeader({ children }: { children: React.ReactNode }) {
  return (
    <th className="h-[var(--u-size-table-row)] border-b border-[var(--u-color-border)] px-2 font-medium">
      {children}
    </th>
  );
}

function Cell({
  children,
  muted = false,
  strong = false,
}: {
  children: React.ReactNode;
  muted?: boolean;
  strong?: boolean;
}) {
  return (
    <td className="h-[var(--u-size-table-row)] truncate px-2 text-[var(--u-color-text)]">
      <span className={muted ? "text-[var(--u-color-text-soft)]" : strong ? "font-medium" : undefined}>
        {children}
      </span>
    </td>
  );
}

function Property({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid min-h-[var(--u-size-table-row)] grid-cols-[96px_minmax(0,1fr)] items-center gap-2 border-b border-[var(--u-color-border)]">
      <span className="text-[var(--u-color-text-soft)]">{label}</span>
      <span className="truncate text-[var(--u-color-text)]">{value}</span>
    </div>
  );
}

function qualifiedTableName(table: DatabaseTable) {
  return table.schema ? `${table.schema}.${table.name}` : table.name;
}
