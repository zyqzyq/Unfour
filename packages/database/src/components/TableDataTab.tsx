import { ChevronLeft, ChevronRight, Code2, Database, Download, Plus, RefreshCw, RotateCcw } from "lucide-react";
import { useState } from "react";
import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseQueryResult,
  DatabaseTable,
  DatabaseTableColumn,
} from "@unfour/command-client";
import {
  Button,
  ConfirmDialog,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  EmptyState,
  ErrorState,
  IconButton,
  Input,
  Select,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import type { DatabaseTableViewState, TableEditing } from "../model/types";
import { buildPreviewSql } from "../result-utils";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";
import { TableDataGrid } from "./TableDataGrid";
import { TableExportDialog } from "./TableExportDialog";

export function TableDataTab({
  connection,
  editing,
  error,
  executePending,
  loading,
  onPageChange,
  onRefresh,
  onSwitchToStructure,
  onTableFilter,
  onTableSort,
  result,
  table,
  tableFilter,
  tableSort,
  tableView,
}: {
  connection?: DatabaseConnection | null;
  editing?: TableEditing | null;
  error?: unknown;
  executePending: boolean;
  loading?: boolean;
  onPageChange: (pageIndex: number, pageSize: number) => void;
  onRefresh: () => void;
  onSwitchToStructure?: () => void;
  onTableFilter: (filter: string) => void;
  onTableSort: (column: string) => void;
  result: DatabaseQueryResult | null;
  table?: DatabaseTable | null;
  tableFilter: string;
  tableSort: { column: string; descending: boolean } | null;
  tableView: DatabaseTableViewState | null;
}) {
  const { t } = useI18n();
  const [addOpen, setAddOpen] = useState(false);
  const [applyOpen, setApplyOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const isLoading = Boolean(loading && (!result || !tableView));
  const tableMeta = table ?? null;

  if (error && !result && !isLoading) {
    return (
      <ErrorState className="m-2 min-h-0 flex-1">
        <DatabaseErrorDetails error={error} />
      </ErrorState>
    );
  }
  if (!result && !tableView && !isLoading) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.editor.tableDataEmpty")}</EmptyState>;
  }

  const displayName = tableView?.tableName ?? tableMeta?.name ?? "";
  const gridResult = result ?? buildLoadingResult(tableMeta);
  const firstRow = tableView && tableView.totalRows > 0 ? tableView.pageIndex * tableView.pageSize + 1 : 0;
  const lastRow = tableView ? Math.min(tableView.totalRows, (tableView.pageIndex + 1) * tableView.pageSize) : 0;
  const hasPrevious = Boolean(tableView && tableView.pageIndex > 0);
  const hasNext = Boolean(tableView && lastRow < tableView.totalRows);
  const totalPages = tableView ? Math.max(1, Math.ceil(tableView.totalRows / Math.max(1, tableView.pageSize))) : 1;
  const browseSql = tableView ? buildPreviewSql(tableView.tableName, tableView.pageSize, tableView.pageIndex) : "";
  const pendingCount = editing?.pendingChanges.length ?? 0;
  const controlsLocked = executePending || isLoading || pendingCount > 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar className="h-8">
        <ToolbarGroup className="min-w-0">
          <span className="truncate text-[12px] font-medium text-[var(--u-color-text)]">{displayName}</span>
          {!isLoading && tableView ? (
            <span className="text-[11px] text-[var(--u-color-text-soft)]">
              {firstRow}-{lastRow} of {tableView.totalRows}
            </span>
          ) : null}
        </ToolbarGroup>
        <ToolbarGroup>
          <TableExportAction connection={connection} table={tableMeta} />
          {onSwitchToStructure ? (
            <button
              className="inline-flex h-[22px] items-center rounded-[5px] px-2 text-[12px] font-medium text-[var(--u-color-text-muted)] transition-colors duration-150 hover:text-[var(--u-color-text)]"
              onClick={onSwitchToStructure}
              type="button"
            >
              {t("database.editor.structureView")}
            </button>
          ) : null}
          {editing?.canInsert ? (
            <Button
              disabled={editing.pending || isLoading}
              onClick={() => setAddOpen(true)}
              size="sm"
              type="button"
              variant="outline"
            >
              <Plus size={13} />
              {t("database.editing.addRow")}
            </Button>
          ) : null}
          {editing && !editing.canUpdateDelete ? (
            <span className="max-w-[220px] truncate text-[11px] text-[var(--u-color-text-soft)]" title={t("database.editing.primaryKeyRequired")}>
              {t("database.editing.insertOnly")}
            </span>
          ) : null}
          <Select
            aria-label={t("database.grid.pageSizeAria")}
            className="w-[88px]"
            disabled={controlsLocked}
            onChange={(event) => onPageChange(0, Number(event.target.value))}
            options={[
              { label: "50", value: "50" },
              { label: "100", value: "100" },
              { label: "250", value: "250" },
            ]}
            value={tableView ? String(tableView.pageSize) : "50"}
          />
          <IconButton disabled={controlsLocked} label={t("database.grid.refreshPreview")} onClick={onRefresh}>
            <RefreshCw size={13} />
          </IconButton>
        </ToolbarGroup>
      </Toolbar>
      {editing && pendingCount > 0 ? (
        <div className="flex min-h-9 shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[color:color-mix(in_srgb,var(--u-color-warning)_8%,var(--u-color-surface))] px-3">
          <span className="min-w-0 flex-1 text-[12px] text-[var(--u-color-text)]">
            {t("database.editing.pendingChanges", { count: pendingCount })}
          </span>
          <Button disabled={editing.pending} onClick={() => setPreviewOpen(true)} size="sm" type="button" variant="ghost">
            <Code2 size={13} />
            {t("database.editing.previewSql")}
          </Button>
          <Button disabled={editing.pending} onClick={editing.onRevert} size="sm" type="button" variant="ghost">
            <RotateCcw size={13} />
            {t("database.editing.revert")}
          </Button>
          <Button disabled={editing.pending} onClick={() => setApplyOpen(true)} size="sm" type="button">
            {t("database.editing.apply")}
          </Button>
        </div>
      ) : null}
      {isLoading ? (
        <div className="h-0.5 w-full shrink-0 overflow-hidden bg-[var(--u-color-border)]">
          <div className="h-full w-1/3 animate-indeterminate rounded-full bg-[var(--u-color-primary)]" />
        </div>
      ) : null}
      {error && result ? (
        <div className="border-b border-[var(--u-color-danger)] bg-[color:color-mix(in_srgb,var(--u-color-danger)_8%,var(--u-color-surface))] px-3 py-2 text-[12px]" role="alert">
          <DatabaseErrorDetails error={error} />
        </div>
      ) : null}
      <div className="flex min-h-0 flex-1 flex-col">
        <TableDataGrid
          columns={tableMeta?.columns}
          editing={editing}
          loading={isLoading}
          result={gridResult}
          server={{ filter: tableFilter, onFilter: onTableFilter, onSort: onTableSort, sort: tableSort }}
        />
      </div>
      <div className="flex h-9 shrink-0 items-center gap-3 border-t border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3">
        <Database className="shrink-0 text-[var(--u-color-text-soft)]" size={13} />
        <code className="min-w-0 flex-1 truncate font-mono text-[12px] text-[var(--u-color-text-muted)]" title={browseSql}>
          {browseSql}
        </code>
        {tableView ? (
          <div className="flex shrink-0 items-center gap-1 text-[var(--u-color-text-soft)]">
            <IconButton
              disabled={!hasPrevious || controlsLocked}
              label={t("database.grid.prevPage")}
              onClick={() => onPageChange(tableView.pageIndex - 1, tableView.pageSize)}
              size="compact"
            >
              <ChevronLeft size={14} />
            </IconButton>
            <span className="px-1 text-[11px] tabular-nums text-[var(--u-color-text-muted)]">
              {t("database.grid.page", { page: tableView.pageIndex + 1, pages: totalPages })}
            </span>
            <IconButton
              disabled={!hasNext || controlsLocked}
              label={t("database.grid.nextPage")}
              onClick={() => onPageChange(tableView.pageIndex + 1, tableView.pageSize)}
              size="compact"
            >
              <ChevronRight size={14} />
            </IconButton>
          </div>
        ) : null}
      </div>
      {editing ? (
        <>
          <AddRowDialog
            columns={tableMeta?.columns ?? []}
            onOpenChange={setAddOpen}
            onSubmit={(values) => {
              editing.onInsertRow(values);
              setAddOpen(false);
            }}
            open={addOpen}
            pending={editing.pending}
          />
          <ConfirmDialog
            confirmLabel={t("database.editing.apply")}
            description={t("database.editing.applyBody", { count: pendingCount })}
            onConfirm={() => {
              editing.onApply();
              setApplyOpen(false);
            }}
            onOpenChange={setApplyOpen}
            open={applyOpen}
            pending={editing.pending}
            title={t("database.editing.applyTitle")}
          />
          <Dialog onOpenChange={setPreviewOpen} open={previewOpen}>
            <DialogContent title={t("database.editing.previewSql")}>
              <DialogHeader><DialogTitle>{t("database.editing.previewSql")}</DialogTitle></DialogHeader>
              <DialogBody>
                <pre className="max-h-[55vh] overflow-auto whitespace-pre-wrap rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3 font-mono text-[12px] text-[var(--u-color-text)]">
                  {editing.previewSql}
                </pre>
              </DialogBody>
            </DialogContent>
          </Dialog>
        </>
      ) : null}
    </div>
  );
}

function TableExportAction({ connection, table }: { connection?: DatabaseConnection | null; table: DatabaseTable | null }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  if (!connection || !table || table.kind !== "table") return null;
  return <>
    <Button onClick={() => setOpen(true)} size="sm" type="button" variant="outline"><Download size={13} />{t("database.export.export")}</Button>
    {open ? <TableExportDialog connection={connection} onOpenChange={setOpen} table={table} /> : null}
  </>;
}

function buildLoadingResult(tableMeta: DatabaseTable | null): DatabaseQueryResult {
  const columns = tableMeta?.columns ?? [];
  return {
    columns: columns.map((column) => ({ name: column.name, dataType: column.dataType })),
    rows: [],
    affectedRows: 0,
    durationMs: 0,
    safety: { classification: "select", requiresConfirmation: false, confirmed: false, message: null },
  };
}

function defaultMode(column: DatabaseTableColumn): DatabaseCellValue["mode"] {
  if (column.generated || column.autoIncrement || column.defaultValue != null) return "default";
  return column.nullable ? "null" : "value";
}

function AddRowDialog({
  columns,
  onOpenChange,
  onSubmit,
  open,
  pending,
}: {
  columns: DatabaseTableColumn[];
  onOpenChange: (open: boolean) => void;
  onSubmit: (values: DatabaseCellValue[]) => void;
  open: boolean;
  pending: boolean;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [modes, setModes] = useState<Record<string, DatabaseCellValue["mode"]>>({});

  function modeFor(column: DatabaseTableColumn) {
    return modes[column.name] ?? defaultMode(column);
  }

  function submit() {
    const values = columns.map((column): DatabaseCellValue => {
      const mode = modeFor(column);
      return mode === "value"
        ? { column: column.name, mode, value: draft[column.name] ?? "" }
        : { column: column.name, mode, value: null };
    });
    onSubmit(values);
    setDraft({});
    setModes({});
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent title={t("database.editing.addRow")}>
        <DialogHeader><DialogTitle>{t("database.editing.addRow")}</DialogTitle></DialogHeader>
        <DialogBody className="max-h-[60vh] space-y-2 overflow-auto">
          {columns.map((column) => {
            const mode = modeFor(column);
            const options = [
              { label: t("database.editing.value"), value: "value" },
              ...(column.nullable ? [{ label: "NULL", value: "null" }] : []),
              ...(column.generated || column.autoIncrement || column.defaultValue != null
                ? [{ label: "DEFAULT", value: "default" }]
                : []),
            ];
            return (
              <label className="grid grid-cols-[minmax(120px,1fr)_100px_minmax(160px,2fr)] items-center gap-2" key={column.name}>
                <span className="min-w-0 truncate text-[11px] font-medium text-[var(--u-color-text-soft)]" title={column.name}>
                  {column.name}
                </span>
                <Select
                  aria-label={t("database.editing.columnMode", { column: column.name })}
                  disabled={column.generated}
                  onChange={(event) => setModes((current) => ({ ...current, [column.name]: event.target.value as DatabaseCellValue["mode"] }))}
                  options={options}
                  value={mode}
                />
                <Input
                  aria-label={t("database.editing.columnValue", { column: column.name })}
                  disabled={mode !== "value"}
                  onChange={(event) => setDraft((current) => ({ ...current, [column.name]: event.target.value }))}
                  placeholder={mode === "value" ? t("database.editing.emptyStringAllowed") : mode.toUpperCase()}
                  value={draft[column.name] ?? ""}
                />
              </label>
            );
          })}
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">
            {t("common.confirm.cancel")}
          </Button>
          <Button disabled={pending} onClick={submit} size="sm" type="button">
            {t("database.editing.stageInsert")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
