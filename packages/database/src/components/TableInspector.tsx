import { Play, RefreshCw } from "lucide-react";
import type { DatabaseTable, DatabaseTableStructure } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, IconButton, LoadingState, StatusBadge, Tabs, Toolbar, ToolbarGroup } from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

type StructureTab = "columns" | "indexes" | "constraints" | "properties" | "ddl";

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
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar className="h-8">
        <ToolbarGroup className="min-w-0">
          <span className="truncate text-[12px] font-semibold text-[var(--u-color-text)]">
            {table ? qualifiedTableName(table) : "Table Structure"}
          </span>
          {table ? <StatusBadge>{table.kind}</StatusBadge> : null}
        </ToolbarGroup>
        <ToolbarGroup>
          <IconButton disabled={loading} label="Refresh table structure" onClick={onRefresh}>
            <RefreshCw size={13} />
          </IconButton>
          <Button disabled={!table || previewPending} onClick={onPreview} size="sm" type="button">
            <Play size={13} />
            Preview
          </Button>
        </ToolbarGroup>
      </Toolbar>
      <Tabs
        activeId={activeTab}
        className="h-[30px] px-1"
        onSelect={(tabId) => onSelectTab(tabId as StructureTab)}
        tabs={[
          { id: "columns", title: "Columns" },
          { id: "indexes", title: "Indexes" },
          { id: "constraints", title: "Constraints" },
          { id: "properties", title: "Properties" },
          { id: "ddl", title: "DDL" },
        ]}
      />
      {renderTableInspectorContent({ activeTab, error, loading, structure, table })}
    </div>
  );
}

function renderTableInspectorContent({
  activeTab,
  error,
  loading,
  structure,
  table,
}: {
  activeTab: StructureTab;
  error?: unknown;
  loading: boolean;
  structure?: DatabaseTableStructure | null;
  table: DatabaseTable | null;
}) {
  if (error) {
    return (
      <ErrorState className="m-2 min-h-0 flex-1">
        <DatabaseErrorDetails error={error} />
      </ErrorState>
    );
  }

  if (loading) {
    return <LoadingState className="m-2 min-h-0 flex-1">Loading table structure...</LoadingState>;
  }

  if (!table) {
    return <EmptyState className="m-2 min-h-0 flex-1">Select a table from the connection tree to view its structure.</EmptyState>;
  }

  // Prefer the on-demand structure (with defaults) but fall back to the
  // lightweight schema columns while it loads.
  const columns = structure?.columns ?? table.columns;

  if (activeTab === "columns") {
    if (!columns.length) {
      return <EmptyState className="m-2 min-h-0 flex-1">No columns were returned for this table.</EmptyState>;
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
              <ColumnHeader>Name</ColumnHeader>
              <ColumnHeader>Type</ColumnHeader>
              <ColumnHeader>Nullable</ColumnHeader>
              <ColumnHeader>Default</ColumnHeader>
              <ColumnHeader>Key</ColumnHeader>
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
                    {column.nullable ? "yes" : "no"}
                  </StatusBadge>
                </Cell>
                {column.defaultValue != null ? (
                  <Cell>{column.defaultValue}</Cell>
                ) : (
                  <Cell muted>—</Cell>
                )}
                <Cell>{column.primaryKey ? <StatusBadge tone="success">PK</StatusBadge> : ""}</Cell>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  if (activeTab === "indexes") {
    const indexes = structure?.indexes ?? [];
    if (!indexes.length) {
      return <EmptyState className="m-2 min-h-0 flex-1">No indexes were found for this table.</EmptyState>;
    }
    return (
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full table-fixed text-left text-[12px]">
          <colgroup>
            <col className="w-[34%]" />
            <col className="w-[40%]" />
            <col className="w-[13%]" />
            <col className="w-[13%]" />
          </colgroup>
          <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
            <tr>
              <ColumnHeader>Name</ColumnHeader>
              <ColumnHeader>Columns</ColumnHeader>
              <ColumnHeader>Unique</ColumnHeader>
              <ColumnHeader>Primary</ColumnHeader>
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
                <Cell>{index.unique ? <StatusBadge tone="success">yes</StatusBadge> : ""}</Cell>
                <Cell>{index.primary ? <StatusBadge tone="success">PK</StatusBadge> : ""}</Cell>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  if (activeTab === "constraints") {
    const foreignKeys = structure?.foreignKeys ?? [];
    if (!foreignKeys.length) {
      return <EmptyState className="m-2 min-h-0 flex-1">No foreign keys were found for this table.</EmptyState>;
    }
    return (
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full table-fixed text-left text-[12px]">
          <colgroup>
            <col className="w-[28%]" />
            <col className="w-[32%]" />
            <col className="w-[40%]" />
          </colgroup>
          <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
            <tr>
              <ColumnHeader>Name</ColumnHeader>
              <ColumnHeader>Columns</ColumnHeader>
              <ColumnHeader>References</ColumnHeader>
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
      </div>
    );
  }

  if (activeTab === "properties") {
    return (
      <div className="min-h-0 flex-1 overflow-auto p-2 text-[12px]">
        <Property label="Schema" value={table.schema ?? "default"} />
        <Property label="Name" value={table.name} />
        <Property label="Kind" value={table.kind} />
        <Property label="Columns" value={String(columns.length)} />
        <Property label="Indexes" value={String(structure?.indexes.length ?? 0)} />
        <Property label="Foreign keys" value={String(structure?.foreignKeys.length ?? 0)} />
      </div>
    );
  }

  // ddl
  if (!structure?.ddl) {
    return <EmptyState className="m-2 min-h-0 flex-1">DDL is not available for this object.</EmptyState>;
  }
  return (
    <div className="min-h-0 flex-1 overflow-auto p-2">
      <pre className="whitespace-pre-wrap break-words rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2 font-mono text-[12px] text-[var(--u-color-text)]">
        {structure.ddl}
      </pre>
    </div>
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
