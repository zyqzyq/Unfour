import { Clipboard, Eye, Search, Trash2 } from "lucide-react";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import type { DatabaseQueryResult, DatabaseTableColumn } from "@unfour/command-client";
import {
  Button,
  DataTable,
  Dialog,
  DialogBody,
  DialogContent,
  DialogHeader,
  DialogTitle,
  IconButton,
  Input,
  Select,
  StatusBadge,
  useI18n,
  type DataTableColumn,
  type DataTableSelection,
} from "@unfour/ui";
import type { PendingTableChange, TableEditing } from "../model/types";
import { databaseValue, pendingValue } from "../model/table-row-editing";
import { serializeDatabaseCell, serializeDatabaseRow, tryFormatJson } from "../result-utils";
import { QueryResultPagination } from "./QueryResultPagination";
import { filterQueryResultRows, type QueryResultView } from "./query-result-view";
import {
  calculateAutoFitWidth,
  copyStatusLabel,
  isLikelyJson,
  MAX_CELL_TITLE_LENGTH,
  MAX_VALUE_VIEWER_LENGTH,
  renderCell,
  renderServerSortIcon,
  renderSortIcon,
  truncateText,
  type SortState,
} from "./table-data-grid-helpers";

const MAX_RENDERED_TABLE_ROWS = 500;
const QUERY_RESULT_PAGE_SIZE = 100;
const EMPTY_PENDING_CHANGES: PendingTableChange[] = [];
const ROW_ACTION_COLUMN_WIDTH = 72;

type CellViewer = { columnName: string; value: string | null };
type DataRow = Array<string | null> & { pendingRowKey?: string };
type EditTarget = { row: DataRow; columnIndex: number };
type IndexedPendingChange = {
  change: PendingTableChange;
  valuesByColumn: Map<string, PendingTableChange["values"][number]>;
};

function selectedDataCell(selection: DataTableSelection | null) {
  // Grid column zero contains row actions, outside the result's data columns.
  if (!selection || selection.endCol === 0) return null;
  return { row: selection.endRow, column: selection.endCol - 1 };
}

type ServerControls = {
  sort: { column: string; descending: boolean } | null;
  filter: string;
  onSort: (column: string) => void;
  onFilter: (filter: string) => void;
};

function indexedPendingValue(
  indexedChange: IndexedPendingChange | undefined,
  columnName: string,
  fallback: string | null,
) {
  const cell = indexedChange?.valuesByColumn.get(columnName);
  if (!cell) return fallback;
  if (cell.mode === "null") return null;
  if (cell.mode === "default") return "DEFAULT";
  return cell.value ?? "";
}

export function TableDataGrid({
  columns,
  editing,
  loading,
  queryView,
  result,
  server,
}: {
  columns?: DatabaseTableColumn[] | null;
  editing?: TableEditing | null;
  loading?: boolean;
  queryView?: QueryResultView;
  result: DatabaseQueryResult;
  server?: ServerControls | null;
}) {
  const { t } = useI18n();
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied-cell" | "copied-row" | "failed">("idle");
  const [localFilter, setLocalFilter] = useState("");
  const [localSort, setLocalSort] = useState<SortState | null>(null);
  const [localPage, setLocalPage] = useState(0);
  const { filter, sort, page, onFilter, onSort, onPage } = queryView ?? {
    filter: localFilter, sort: localSort, page: localPage,
    onFilter: setLocalFilter, onSort: setLocalSort, onPage: setLocalPage,
  };
  const viewport = useRef<HTMLDivElement>(null);
  const localResult = server ? null : result;
  useLayoutEffect(() => {
    if (localResult && viewport.current) viewport.current.scrollTop = 0;
  }, [page, filter, sort, localResult]);
  const [viewer, setViewer] = useState<CellViewer | null>(null);
  const [viewerRaw, setViewerRaw] = useState(false);
  const [edit, setEdit] = useState<EditTarget | null>(null);
  const [editValue, setEditValue] = useState("");
  const [editMode, setEditMode] = useState<"value" | "null">("value");
  const [selection, setSelection] = useState<DataTableSelection | null>(null);
  const activeCell = selectedDataCell(selection);
  const [columnsWidths, setColumnsWidths] = useState<Record<string, number>>(() => ({
    __row_actions: ROW_ACTION_COLUMN_WIDTH,
  }));
  const currentResult = loading ? null : result;
  const [previousResult, setPreviousResult] = useState(currentResult);
  if (previousResult !== currentResult) {
    setPreviousResult(currentResult);
    setLocalPage(0);
    setLocalFilter("");
    setLocalSort(null);
    setSelection(null);
    setViewer(null);
    setEdit(null);
    setCopyStatus("idle");
  }

  const viewerValue = viewer?.value ?? null;
  const viewerJson = viewerValue !== null && viewerValue.length <= MAX_VALUE_VIEWER_LENGTH
    ? tryFormatJson(viewerValue)
    : null;
  const viewerIsJson = Boolean(viewerJson?.isJson || (viewerValue && isLikelyJson(viewerValue)));
  const viewerContent = viewerJson?.isJson && !viewerRaw ? viewerJson.formatted : viewerValue;
  const viewerPreview = viewerContent === null ? null : truncateText(viewerContent, MAX_VALUE_VIEWER_LENGTH);
  const viewerTruncated = Boolean(viewerContent && viewerContent.length > MAX_VALUE_VIEWER_LENGTH);
  const isLoading = Boolean(loading);
  const insertedRows = useMemo<DataRow[]>(
    () =>
      (editing?.pendingChanges ?? [])
        .filter((change) => change.operation === "insert")
        .map((change) => {
          const row = result.columns.map((column) => {
            const cell = change.values.find((candidate) => candidate.column === column.name);
            if (!cell || cell.mode === "null") return null;
            if (cell.mode === "default") return "DEFAULT";
            return cell.value ?? "";
          }) as DataRow;
          row.pendingRowKey = change.rowKey;
          return row;
        }),
    [editing?.pendingChanges, result.columns],
  );

  const queryRows = queryView?.rows;
  const processedRows = useMemo<DataRow[]>(() => {
    if (queryRows) return queryRows;
    const rows = insertedRows.length ? [...result.rows, ...insertedRows] : result.rows;
    return server ? rows : filterQueryResultRows(rows, filter, sort);
  }, [queryRows, filter, insertedRows, result.rows, server, sort]);
  const pageCount = Math.max(1, Math.ceil(processedRows.length / QUERY_RESULT_PAGE_SIZE));
  const pageIndex = Math.min(page, pageCount - 1);
  const offset = server ? 0 : pageIndex * QUERY_RESULT_PAGE_SIZE;
  const visibleRows = useMemo(
    () => processedRows.slice(offset, offset + (server ? MAX_RENDERED_TABLE_ROWS : QUERY_RESULT_PAGE_SIZE)),
    [processedRows, offset, server],
  );
  const pendingChanges = editing?.pendingChanges ?? EMPTY_PENDING_CHANGES;
  const pendingChangesByRowKey = useMemo(
    () => new Map(pendingChanges.map((change) => [
      change.rowKey,
      {
        change,
        valuesByColumn: new Map(change.values.map((cell) => [cell.column, cell])),
      } satisfies IndexedPendingChange,
    ])),
    [pendingChanges],
  );
  const rowKeyForEditing = editing?.rowKey;
  const visibleRowKeys = useMemo(
    () => visibleRows.map(
      (row) => row.pendingRowKey ?? rowKeyForEditing?.(row) ?? JSON.stringify(row),
    ),
    [rowKeyForEditing, visibleRows],
  );
  const autoFitWidths = useMemo(
    () => result.columns.map((column, columnIndex) =>
      calculateAutoFitWidth(
        column,
        visibleRows.map((row, rowIndex) => indexedPendingValue(
          pendingChangesByRowKey.get(visibleRowKeys[rowIndex]),
          column.name,
          row[columnIndex] ?? null,
        )),
      )),
    [pendingChangesByRowKey, result.columns, visibleRowKeys, visibleRows],
  );
  const hasPendingChanges = pendingChanges.length > 0;

  function displayedValue(row: DataRow, rowIndex: number, columnIndex: number) {
    return indexedPendingValue(
      pendingChangesByRowKey.get(visibleRowKeys[rowIndex]),
      result.columns[columnIndex].name,
      row[columnIndex] ?? null,
    );
  }

  async function copyText(text: string, status: "copied-cell" | "copied-row") {
    try {
      await navigator.clipboard.writeText(text);
      setCopyStatus(status);
      window.setTimeout(() => setCopyStatus("idle"), 1400);
    } catch {
      setCopyStatus("failed");
    }
  }

  function changePage(nextPage: number) {
    onPage(nextPage);
    setSelection(null);
  }

  function changeFilter(value: string) {
    changePage(0);
    onFilter(value);
  }

  function changeSort(value: SortState | null) {
    changePage(0);
    onSort(value);
  }

  function toggleSort(columnIndex: number) {
    if (sort?.columnIndex !== columnIndex) changeSort({ columnIndex, direction: "asc" });
    else changeSort(sort.direction === "asc" ? { columnIndex, direction: "desc" } : null);
  }

  function commitEdit() {
    if (!edit || !editing) {
      setEdit(null);
      return;
    }
    const columnName = result.columns[edit.columnIndex]?.name;
    if (columnName) {
      const rowKey = edit.row.pendingRowKey ?? editing.rowKey(edit.row);
      const original = edit.row[edit.columnIndex] ?? null;
      const current = pendingValue(editing.pendingChanges, rowKey, columnName, original);
      const next = editMode === "null" ? databaseValue(columnName, null) : databaseValue(columnName, editValue);
      if (next.mode === "null" ? current !== null : current !== next.value) {
        editing.onUpdateCell(edit.row, columnName, next, rowKey);
      }
    }
    setEdit(null);
  }

  const rowActionColumn: DataTableColumn<DataRow> = {
    // Editing becomes available only after the initial table view has loaded.
    // Reserve both action slots up front so DataTable's initial width cache
    // cannot clip the delete action when editing appears.
    autoFitWidth: ROW_ACTION_COLUMN_WIDTH,
    copyValue: false,
    cell: (row, rowIndex) => {
      const rowKey = row.pendingRowKey ?? editing?.rowKey(row);
      const deleted = rowKey
        ? pendingChangesByRowKey.get(rowKey)?.change.operation === "delete"
        : false;
      return (
        <div className="flex items-center gap-0.5">
          <IconButton
            label={t("database.grid.copyRow", { row: offset + rowIndex + 1 })}
            onClick={() => {
              const values = result.columns.map((_, index) => displayedValue(row, rowIndex, index));
              void copyText(serializeDatabaseRow(result, values, "\t"), "copied-row");
            }}
            size="compact"
          >
            <Clipboard size={12} />
          </IconButton>
          {editing ? (
            <IconButton
              disabled={(!editing.canUpdateDelete && !row.pendingRowKey) || editing.pending || deleted}
              label={t("database.editing.deleteRow")}
              onClick={() => editing.onDeleteRow(row, rowKey)}
              size="compact"
            >
              <Trash2 className="text-[var(--u-color-danger)]" size={12} />
            </IconButton>
          ) : null}
        </div>
      );
    },
    header: "#",
    id: "__row_actions",
    width: columnsWidths.__row_actions ?? ROW_ACTION_COLUMN_WIDTH,
  };

  const columnIds = result.columns.map((column, index) =>
    server && column.name ? column.name : `result-column-${index}`);
  const dataColumns: DataTableColumn<DataRow>[] = [
    rowActionColumn,
    ...result.columns.map((column, columnIndex) => ({
      autoFitWidth: autoFitWidths[columnIndex],
      copyValue: (row: DataRow, rowIndex: number) => displayedValue(row, rowIndex, columnIndex),
      cell: (row: DataRow, rowIndex: number) => {
        const rowKey = row.pendingRowKey ?? editing?.rowKey(row) ?? JSON.stringify(row);
        const indexedChange = pendingChangesByRowKey.get(rowKey);
        const change = indexedChange?.change;
        const value = displayedValue(row, rowIndex, columnIndex);
        const changed = indexedChange?.valuesByColumn.has(column.name);
        const deleted = change?.operation === "delete";
        const active = activeCell?.row === rowIndex && activeCell.column === columnIndex;
        if (edit?.row === row && edit.columnIndex === columnIndex) {
          return (
            <div
              className="flex min-w-[180px] items-center gap-1"
              onBlur={(event) => {
                if (!event.currentTarget.contains(event.relatedTarget)) commitEdit();
              }}
            >
              <Select
                aria-label={t("database.editing.valueMode")}
                className="h-6 w-[76px]"
                onChange={(event) => setEditMode(event.target.value as "value" | "null")}
                options={[
                  { label: t("database.editing.value"), value: "value" },
                  { label: "NULL", value: "null" },
                ]}
                value={editMode}
              />
              <input
                autoFocus
                className="block min-w-0 flex-1 rounded-sm border border-[var(--u-color-focus)] bg-[var(--u-color-surface)] px-1 font-mono text-[12px] text-[var(--u-color-text)] focus-visible:outline-none disabled:opacity-50"
                disabled={editMode === "null"}
                onChange={(event) => setEditValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    commitEdit();
                  } else if (event.key === "Escape") {
                    event.preventDefault();
                    setEdit(null);
                  }
                }}
                value={editValue}
              />
            </div>
          );
        }
        return (
          <button
            className={[
              "block w-full cursor-pointer truncate text-left font-mono text-[12px] text-[var(--u-color-text)] focus-visible:outline-none",
              active ? "ring-1 ring-inset ring-[var(--u-color-focus)]" : "focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]",
              changed ? "bg-[color:color-mix(in_srgb,var(--u-color-warning)_15%,transparent)]" : "",
              deleted ? "line-through opacity-45" : "",
            ].join(" ")}
            onDoubleClick={
              editing &&
              (editing.canUpdateDelete || Boolean(row.pendingRowKey)) &&
              !columns?.find((candidate) => candidate.name === column.name)?.generated &&
              !editing.pending &&
              !deleted
                ? () => {
                    setEditValue(value ?? "");
                    setEditMode(value === null ? "null" : "value");
                    setEdit({ row, columnIndex });
                  }
                : undefined
            }
            title={
              editing && (editing.canUpdateDelete || row.pendingRowKey)
                ? t("database.editing.editHint")
                : (value === null ? "NULL" : truncateText(value, MAX_CELL_TITLE_LENGTH))
            }
            type="button"
          >
            {renderCell(value)}
          </button>
        );
      },
      header: (
        <button
          className="group/header flex w-full min-w-0 cursor-pointer items-center gap-1 text-left hover:text-[var(--u-color-text)] focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
          disabled={hasPendingChanges}
          onClick={() => (server ? server.onSort(column.name) : toggleSort(columnIndex))}
          title={t("database.grid.sortBy", { column: column.name })}
          type="button"
        >
          <span className="truncate">{column.name}</span>
          {server ? renderServerSortIcon(server.sort, column.name) : renderSortIcon(sort, columnIndex)}
        </button>
      ),
      id: columnIds[columnIndex],
      meta: column.dataType,
      width: columnsWidths[columnIds[columnIndex]] ?? Math.min(Math.max(column.name.length * 9 + 96, 140), 360),
    })),
  ];

  const gridRows = isLoading ? [] : visibleRows;

  return (
    <div
      aria-busy={isLoading}
      className={`flex min-w-0 flex-1 flex-col ${queryView ? "min-h-[calc(var(--u-size-section-toolbar)+2rem+3*var(--u-size-table-row))]" : "min-h-0"}`}
    >
      <div className="flex min-h-11 shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 py-1.5">
        <Search className="text-[var(--u-color-text-soft)]" size={13} />
        <Input
          aria-label={t("database.grid.filterPlaceholder")}
          className="h-7 max-w-[320px] text-[12px]"
          disabled={isLoading || hasPendingChanges}
          onChange={(event) => (server ? server.onFilter(event.target.value) : changeFilter(event.target.value))}
          placeholder={t("database.grid.filterPlaceholder")}
          value={server ? server.filter : filter}
        />
        {!server && sort ? (
          <button
            className="text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
            onClick={() => changeSort(null)}
            type="button"
          >
            {t("database.grid.clearSort")}
          </button>
        ) : null}
      </div>
      <DataTable
        className="flex-1 bg-[var(--u-color-canvas)]"
        ref={viewport}
        columns={dataColumns}
        empty={
          isLoading
            ? <span className="sr-only">{t("common.state.loading")}</span>
            : (server ? server.filter : filter)
              ? t("database.grid.noMatches")
              : t("database.grid.empty")
        }
        getRowKey={(row, index) => row.pendingRowKey ?? editing?.rowKey(row) ?? `${JSON.stringify(row)}:${index}`}
        getColumnResizeLabel={(column) => {
          const index = columnIds.indexOf(column.id);
          const name = column === rowActionColumn ? "#" : result.columns[index]?.name || `#${index + 1}`;
          return t("database.grid.resizeColumn", { column: name });
        }}
        onColumnResize={(columnId, width) => setColumnsWidths((current) => ({ ...current, [columnId]: width }))}
        onSelectionChange={setSelection}
        rows={gridRows}
        selection={selection}
      />
      <div className="flex h-8 shrink-0 items-center justify-between border-t border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-3 text-[length:var(--u-font-size-caption)] tabular-nums text-[var(--u-color-text-muted)]">
        <div className="flex items-center gap-1">
          <span>{copyStatusLabel(copyStatus, t)}</span>
          {activeCell && visibleRows[activeCell.row] && result.columns[activeCell.column] ? (
            <IconButton
              label={t("database.grid.valueViewer")}
              onClick={() => {
                const row = visibleRows[activeCell.row];
                const column = result.columns[activeCell.column];
                setViewerRaw(false);
                setViewer({
                  columnName: column.name,
                  value: displayedValue(row, activeCell.row, activeCell.column),
                });
              }}
              size="compact"
            >
              <Eye size={12} />
            </IconButton>
          ) : null}
        </div>
        {!isLoading && (!server && pageCount > 1 ? (
          <QueryResultPagination count={processedRows.length} onChange={changePage} page={pageIndex} pageSize={QUERY_RESULT_PAGE_SIZE} />
        ) : (
          <span>
            {processedRows.length > visibleRows.length
              ? t("database.grid.showingFirst", { shown: visibleRows.length, total: processedRows.length })
              : t("database.grid.rowsRendered", { count: processedRows.length })}
          </span>
        ))}
      </div>
      <Dialog onOpenChange={(open) => !open && setViewer(null)} open={viewer !== null}>
        <DialogContent title={t("database.grid.valueViewer")}>
          <DialogHeader>
            <DialogTitle>{viewer?.columnName ?? t("database.grid.valueViewer")}</DialogTitle>
          </DialogHeader>
          <DialogBody className="space-y-2">
            {viewer?.value === null ? (
              <StatusBadge>NULL</StatusBadge>
            ) : (
              <>
                {viewerIsJson ? (
                  <div className="flex items-center justify-between">
                    <StatusBadge>JSON</StatusBadge>
                    {viewerJson?.isJson ? (
                      <button
                        className="text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)] hover:text-[var(--u-color-text)]"
                        onClick={() => setViewerRaw((current) => !current)}
                        type="button"
                      >
                        {viewerRaw ? t("database.grid.viewFormatted") : t("database.grid.viewRaw")}
                      </button>
                    ) : null}
                  </div>
                ) : null}
                <div className="max-h-[50vh] overflow-auto rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
                  <pre className="whitespace-pre-wrap break-words p-2 font-mono text-[12px] text-[var(--u-color-text)]">
                    {viewerPreview}
                  </pre>
                </div>
                {viewerTruncated ? (
                  <p className="text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)]" role="status">
                    {t("database.grid.valueTruncated", { count: MAX_VALUE_VIEWER_LENGTH })}
                  </p>
                ) : null}
              </>
            )}
            <div className="flex items-center justify-between">
              <span className="text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)]">{t("database.grid.selectToCopy")}</span>
              <Button
                disabled={viewer?.value == null}
                onClick={() => viewer?.value != null && copyText(serializeDatabaseCell(viewer.value, "\t"), "copied-cell")}
                size="sm"
                type="button"
                variant="outline"
              >
                <Clipboard size={13} />
                {t("database.grid.copyValue")}
              </Button>
            </div>
          </DialogBody>
        </DialogContent>
      </Dialog>
    </div>
  );
}
