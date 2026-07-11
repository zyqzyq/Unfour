import { Clipboard, Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import type { DatabaseCellValue, DatabaseQueryResult, DatabaseTableColumn } from "@unfour/command-client";
import {
  Button,
  ConfirmDialog,
  DataTable,
  Dialog,
  DialogBody,
  DialogContent,
  DialogHeader,
  DialogTitle,
  IconButton,
  Input,
  StatusBadge,
  useI18n,
  type DataTableColumn,
  type DataTableSelection,
} from "@unfour/ui";
import type { TableEditing } from "../model/types";
import { serializeDatabaseCell, serializeDatabaseRow, tryFormatJson } from "../result-utils";
import {
  buildSkeletonColumns,
  buildSkeletonRows,
  compareCells,
  copyStatusLabel,
  renderCell,
  renderServerSortIcon,
  renderSortIcon,
  truncatePreview,
  type SortState,
} from "./table-data-grid-helpers";


const MAX_RENDERED_ROWS = 500;

type CellViewer = { columnName: string; value: string | null };
type EditTarget = { row: Array<string | null>; columnIndex: number };
type DataRow = Array<string | null>;
type PendingUpdate = {
  columnName: string;
  value: string;
  primaryKey: DatabaseCellValue[];
};
type ServerControls = {
  sort: { column: string; descending: boolean } | null;
  filter: string;
  onSort: (column: string) => void;
  onFilter: (filter: string) => void;
};

export function TableDataGrid({
  columns,
  editing,
  loading,
  result,
  server,
}: {
  // Optional table schema used to render the loading skeleton (Navicat-style):
  // the grid paints real column headers immediately while rows stream in.
  columns?: DatabaseTableColumn[] | null;
  editing?: TableEditing | null;
  // Skeleton mode: when true (with `columns`), the grid renders placeholder rows
  // instead of waiting for `result`, so the frame never pops in empty.
  loading?: boolean;
  result: DatabaseQueryResult;
  // When present (table browse), sort and filter are applied server-side across
  // the whole table; the grid reflects state and delegates instead of doing its
  // own page-local sort/filter. Absent for ad-hoc query results.
  server?: ServerControls | null;
}) {
  const { t } = useI18n();
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied-cell" | "copied-row" | "failed">("idle");
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<SortState | null>(null);
  const [activeCell, setActiveCell] = useState<{ row: number; column: number } | null>(null);
  const [viewer, setViewer] = useState<CellViewer | null>(null);
  const [viewerRaw, setViewerRaw] = useState(false);
  const viewerJson = viewer && viewer.value !== null ? tryFormatJson(viewer.value) : null;
  const [edit, setEdit] = useState<EditTarget | null>(null);
  const [editValue, setEditValue] = useState("");
  const [deleteRow, setDeleteRow] = useState<DataRow | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<PendingUpdate | null>(null);
  const [selection, setSelection] = useState<DataTableSelection | null>(null);
  const [columnsWidths, setColumnsWidths] = useState<Record<string, number>>(() => {
    const map: Record<string, number> = {};
    map["__row_actions"] = editing ? 72 : 48;
    return map;
  });

  const isSkeleton = Boolean(loading && columns && columns.length > 0);
  const skeletonRowCount = 12;

  const skeletonRows = useMemo<Array<Array<string | null>>>(() => {
    if (!isSkeleton || !columns) {
      return [];
    }
    return buildSkeletonRows(columns, skeletonRowCount);
  }, [isSkeleton, columns, skeletonRowCount]);

  const skeletonColumns = useMemo<DataTableColumn<Array<string | null>>[]>(() => {
    if (!isSkeleton || !columns) {
      return [];
    }
    return buildSkeletonColumns(columns, columnsWidths);
  }, [isSkeleton, columns, columnsWidths]);

  function buildPrimaryKey(row: DataRow): DatabaseCellValue[] {
    return (editing?.primaryKeyColumns ?? []).map((name) => {
      const index = result.columns.findIndex((column) => column.name === name);
      return { column: name, value: index >= 0 ? row[index] ?? null : null };
    });
  }

  function commitEdit() {
    if (!edit || !editing) {
      setEdit(null);
      return;
    }
    const columnName = result.columns[edit.columnIndex]?.name;
    const original = edit.row[edit.columnIndex] ?? "";
    if (columnName && editValue !== original) {
      // Stage the change behind a confirmation step rather than writing to the
      // database the moment the input blurs.
      setPendingUpdate({
        columnName,
        value: editValue,
        primaryKey: buildPrimaryKey(edit.row),
      });
    }
    setEdit(null);
  }

  const processedRows = useMemo(() => {
    // In server mode the rows arrive already sorted and filtered for the whole
    // table, so the grid renders them as-is.
    if (server) {
      return result.rows;
    }

    const needle = filter.trim().toLowerCase();
    const filtered = needle
      ? result.rows.filter((row) => row.some((value) => (value ?? "").toLowerCase().includes(needle)))
      : result.rows;

    if (!sort) {
      return filtered;
    }

    // Copy before sorting so the source result order is preserved.
    const sorted = [...filtered].sort((left, right) => {
      const a = left[sort.columnIndex];
      const b = right[sort.columnIndex];
      const compared = compareCells(a, b);
      return sort.direction === "asc" ? compared : -compared;
    });
    return sorted;
  }, [filter, result.rows, sort, server]);

  const visibleRows = processedRows.slice(0, MAX_RENDERED_ROWS);

  async function copyText(text: string, status: "copied-cell" | "copied-row") {
    try {
      await navigator.clipboard.writeText(text);
      setCopyStatus(status);
      window.setTimeout(() => setCopyStatus("idle"), 1400);
    } catch {
      setCopyStatus("failed");
    }
  }

  function toggleSort(columnIndex: number) {
    setSort((current) => {
      if (!current || current.columnIndex !== columnIndex) {
        return { columnIndex, direction: "asc" };
      }
      if (current.direction === "asc") {
        return { columnIndex, direction: "desc" };
      }
      return null;
    });
  }

  const rowActionColumn: DataTableColumn<Array<string | null>> = {
    cell: (row, rowIndex) => (
      <div className="flex items-center gap-0.5">
        <IconButton
          label={`Copy row ${rowIndex + 1}`}
          onClick={() => copyText(serializeDatabaseRow(result, row, "\t"), "copied-row")}
          size="compact"
        >
          <Clipboard size={12} />
        </IconButton>
        {editing ? (
          <IconButton
            disabled={editing.pending}
            label={t("database.editing.deleteRow")}
            onClick={() => setDeleteRow(row)}
            size="compact"
          >
            <Trash2 className="text-[var(--u-color-danger)]" size={12} />
          </IconButton>
        ) : null}
      </div>
    ),
    header: "#",
    id: "__row_actions",
    width: columnsWidths["__row_actions"] ?? (editing ? 72 : 48),
  };

  const dataColumns: DataTableColumn<Array<string | null>>[] = [
    rowActionColumn,
    ...result.columns.map((column, columnIndex) => ({
      cell: (row: Array<string | null>, rowIndex: number) => {
        const value = row[columnIndex];
        const isActive = activeCell?.row === rowIndex && activeCell?.column === columnIndex;
        if (edit && edit.row === row && edit.columnIndex === columnIndex) {
          return (
            <input
              autoFocus
              className="block w-full rounded-sm border border-[var(--u-color-focus)] bg-[var(--u-color-surface)] px-1 font-mono text-[12px] text-[var(--u-color-text)] focus-visible:outline-none"
              onBlur={commitEdit}
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
          );
        }
        return (
          <button
            className={
              "block w-full cursor-pointer truncate text-left font-mono text-[12px] text-[var(--u-color-text)] hover:text-[var(--u-color-primary)] focus-visible:outline-none " +
              (isActive
                ? "ring-1 ring-inset ring-[var(--u-color-focus)]"
                : "focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]")
            }
            onClick={() => {
              setActiveCell({ row: rowIndex, column: columnIndex });
              setViewerRaw(false);
              setViewer({ columnName: column.name, value: value ?? null });
            }}
            onDoubleClick={
              editing
                ? () => {
                    setEditValue(value ?? "");
                    setEdit({ row, columnIndex });
                  }
                : undefined
            }
            title={editing ? t("database.editing.editHint") : (value ?? "NULL")}
            type="button"
          >
            {renderCell(value)}
          </button>
        );
      },
      header: (
        <button
          className="group/header flex w-full min-w-0 cursor-pointer items-center gap-1 text-left hover:text-[var(--u-color-text)] focus-visible:outline-none"
          onClick={() => (server ? server.onSort(column.name) : toggleSort(columnIndex))}
          title={t("database.grid.sortBy", { column: column.name })}
          type="button"
        >
          <span className="truncate">{column.name}</span>
          {server ? renderServerSortIcon(server.sort, column.name) : renderSortIcon(sort, columnIndex)}
        </button>
      ),
      id: column.name || `column-${columnIndex}`,
      meta: column.dataType,
      width: columnsWidths[column.name || `column-${columnIndex}`] ?? Math.min(Math.max(column.name.length * 9 + 96, 140), 360),
    })),
  ];

  const gridColumns = isSkeleton ? skeletonColumns : dataColumns;
  const gridRows = isSkeleton ? skeletonRows : visibleRows;

  const totalAfterFilter = processedRows.length;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-2">
        <Search className="text-[var(--u-color-text-soft)]" size={13} />
        <Input
          aria-label={t("database.grid.filterPlaceholder")}
          className="h-6 max-w-[260px]"
          disabled={isSkeleton}
          onChange={(event) => (server ? server.onFilter(event.target.value) : setFilter(event.target.value))}
          placeholder={t("database.grid.filterPlaceholder")}
          value={server ? server.filter : filter}
        />
        {!server && sort ? (
          <button
            className="text-[11px] text-[var(--u-color-text-soft)] hover:text-[var(--u-color-text)]"
            onClick={() => setSort(null)}
            type="button"
          >
            {t("database.grid.clearSort")}
          </button>
        ) : null}
      </div>
      <DataTable
        className="flex-1"
        columns={gridColumns}
        empty={(server ? server.filter : filter) ? t("database.grid.noMatches") : t("database.grid.empty")}
        getRowKey={(_, index) => index}
        onColumnResize={(columnId, width) => {
          setColumnsWidths((prev) => ({ ...prev, [columnId]: width }));
        }}
        onSelectionChange={setSelection}
        rows={gridRows}
        selection={selection}
      />
      <div className="flex h-7 shrink-0 items-center justify-between border-t border-[var(--u-color-border)] px-2 text-[11px] text-[var(--u-color-text-soft)]">
        <span>{copyStatusLabel(copyStatus, t)}</span>
        {isSkeleton ? null : (
          <span>
            {totalAfterFilter > visibleRows.length
              ? t("database.grid.showingFirst", { shown: visibleRows.length, total: totalAfterFilter })
              : t("database.grid.rowsRendered", { count: totalAfterFilter })}
          </span>
        )}
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
                {viewerJson?.isJson ? (
                  <div className="flex items-center justify-between">
                    <StatusBadge>JSON</StatusBadge>
                    <button
                      className="text-[11px] text-[var(--u-color-text-soft)] hover:text-[var(--u-color-text)]"
                      onClick={() => setViewerRaw((current) => !current)}
                      type="button"
                    >
                      {viewerRaw ? t("database.grid.viewFormatted") : t("database.grid.viewRaw")}
                    </button>
                  </div>
                ) : null}
                <div className="max-h-[50vh] overflow-auto rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
                  <pre className="whitespace-pre-wrap break-words p-2 font-mono text-[12px] text-[var(--u-color-text)]">
                    {viewerJson?.isJson && !viewerRaw ? viewerJson.formatted : viewer?.value}
                  </pre>
                </div>
              </>
            )}
            <div className="flex items-center justify-between">
              <span className="text-[11px] text-[var(--u-color-text-soft)]">
                {t("database.grid.selectToCopy")}
              </span>
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
      {editing ? (
        <ConfirmDialog
          confirmLabel={t("database.editing.deleteRow")}
          description={t("database.editing.deleteRowBody")}
          onConfirm={() => {
            if (deleteRow) {
              editing.onDeleteRow(buildPrimaryKey(deleteRow));
            }
            setDeleteRow(null);
          }}
          onOpenChange={(open) => !open && setDeleteRow(null)}
          open={deleteRow !== null}
          pending={editing.pending}
          title={t("database.editing.deleteRowTitle")}
        />
      ) : null}
      {editing ? (
        <ConfirmDialog
          confirmLabel={t("database.editing.confirmUpdate")}
          description={t("database.editing.updateCellBody", {
            column: pendingUpdate?.columnName ?? "",
            value: pendingUpdate ? truncatePreview(pendingUpdate.value) : "",
          })}
          onConfirm={() => {
            if (pendingUpdate) {
              editing.onUpdateCell(pendingUpdate.columnName, pendingUpdate.value, pendingUpdate.primaryKey);
            }
            setPendingUpdate(null);
          }}
          onOpenChange={(open) => !open && setPendingUpdate(null)}
          open={pendingUpdate !== null}
          pending={editing.pending}
          title={t("database.editing.updateCellTitle")}
        />
      ) : null}
    </div>
  );
}
