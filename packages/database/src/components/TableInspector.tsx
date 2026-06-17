import { Play, RefreshCw } from "lucide-react";
import type { DatabaseTable } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, IconButton, LoadingState, StatusBadge, Tabs, Toolbar, ToolbarGroup } from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

export function TableInspector({
  activeTab,
  error,
  loading = false,
  onPreview,
  onRefresh,
  onSelectTab,
  previewPending = false,
  table,
}: {
  activeTab: "columns" | "indexes" | "constraints" | "properties" | "ddl";
  error?: unknown;
  loading?: boolean;
  onPreview?: () => void;
  onRefresh?: () => void;
  onSelectTab: (tab: "columns" | "indexes" | "constraints" | "properties" | "ddl") => void;
  previewPending?: boolean;
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
        onSelect={(tabId) => onSelectTab(tabId as "columns" | "indexes" | "constraints" | "properties" | "ddl")}
        tabs={[
          { id: "columns", title: "Columns" },
          { id: "indexes", title: "Indexes" },
          { id: "constraints", title: "Constraints" },
          { id: "properties", title: "Properties" },
          { id: "ddl", title: "DDL" },
        ]}
      />
      {renderTableInspectorContent({ activeTab, error, loading, table })}
    </div>
  );
}

function renderTableInspectorContent({
  activeTab,
  error,
  loading,
  table,
}: {
  activeTab: "columns" | "indexes" | "constraints" | "properties" | "ddl";
  error?: unknown;
  loading: boolean;
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

  if (activeTab === "columns") {
    if (!table.columns.length) {
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
            {table.columns.map((column) => (
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
                <Cell muted>Unavailable</Cell>
                <Cell>{column.primaryKey ? <StatusBadge tone="success">PK</StatusBadge> : ""}</Cell>
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
        <Property label="Columns" value={String(table.columns.length)} />
      </div>
    );
  }

  return (
    <EmptyState className="m-2 min-h-0 flex-1">
      {activeTab === "indexes"
        ? "Index metadata is not exposed by the current backend."
        : "This metadata is not exposed by the current backend."}
    </EmptyState>
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
