import { ArrowDown, ArrowUp, ChevronsUpDown, Clipboard, Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import type { DatabaseCellValue, DatabaseQueryResult } from "@unfour/command-client";
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
} from "@unfour/ui";
import type { TableEditing } from "../model/types";
import { serializeDatabaseCell, serializeDatabaseRow, tryFormatJson } from "../result-utils";

const MAX_RENDERED_ROWS = 500;

type SortState = { columnIndex: number; direction: "asc" | "desc" };
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
  editing,
  result,
  server,
}: {
  editing?: TableEditing | null;
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
  const [viewer, setViewer] = useState<CellViewer | null>(null);
  const [viewerRaw, setViewerRaw] = useState(false);
  const viewerJson = viewer && viewer.value !== null ? tryFormatJson(viewer.value) : null;
  const [edit, setEdit] = useState<EditTarget | null>(null);
  const [editValue, setEditValue] = useState("");
  const [deleteRow, setDeleteRow] = useState<DataRow | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<PendingUpdate | null>(null);

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
    width: editing ? 72 : 48,
  };

  const columns: DataTableColumn<Array<string | null>>[] = [
    rowActionColumn,
    ...result.columns.map((column, columnIndex) => ({
      cell: (row: Array<string | null>) => {
        const value = row[columnIndex];
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
            className="block w-full cursor-pointer truncate text-left font-mono text-[12px] text-[var(--u-color-text)] hover:text-[var(--u-color-primary)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]"
            onClick={() => {
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
          className="flex w-full min-w-0 cursor-pointer items-center gap-1 text-left hover:text-[var(--u-color-text)] focus-visible:outline-none"
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
      width: Math.min(Math.max(column.name.length * 9 + 96, 140), 360),
    })),
  ];

  const totalAfterFilter = processedRows.length;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-2">
        <Search className="text-[var(--u-color-text-soft)]" size={13} />
        <Input
          aria-label={t("database.grid.filterPlaceholder")}
          className="h-6 max-w-[260px]"
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
        columns={columns}
        empty={(server ? server.filter : filter) ? t("database.grid.noMatches") : t("database.grid.empty")}
        getRowKey={(_, index) => index}
        rows={visibleRows}
      />
      <div className="flex h-7 shrink-0 items-center justify-between border-t border-[var(--u-color-border)] px-2 text-[11px] text-[var(--u-color-text-soft)]">
        <span>{copyStatusLabel(copyStatus, t)}</span>
        <span>
          {totalAfterFilter > visibleRows.length
            ? t("database.grid.showingFirst", { shown: visibleRows.length, total: totalAfterFilter })
            : t("database.grid.rowsRendered", { count: totalAfterFilter })}
        </span>
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
                <pre className="max-h-[50vh] overflow-auto whitespace-pre-wrap break-words rounded border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2 font-mono text-[12px] text-[var(--u-color-text)]">
                  {viewerJson?.isJson && !viewerRaw ? viewerJson.formatted : viewer?.value}
                </pre>
              </>
            )}
            <div className="flex justify-end">
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

function truncatePreview(value: string) {
  const text = value.length === 0 ? "''" : value;
  return text.length > 120 ? `${text.slice(0, 120)}…` : text;
}

function compareCells(a: string | null, b: string | null) {
  // NULLs always sort to the end regardless of direction.
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  const numA = Number(a);
  const numB = Number(b);
  if (!Number.isNaN(numA) && !Number.isNaN(numB) && a.trim() !== "" && b.trim() !== "") {
    return numA - numB;
  }
  return a.localeCompare(b);
}

function renderSortIcon(sort: SortState | null, columnIndex: number) {
  if (!sort || sort.columnIndex !== columnIndex) {
    return <ChevronsUpDown className="shrink-0 text-[var(--u-color-text-soft)]" size={12} />;
  }
  return sort.direction === "asc" ? (
    <ArrowUp className="shrink-0 text-[var(--u-color-primary)]" size={12} />
  ) : (
    <ArrowDown className="shrink-0 text-[var(--u-color-primary)]" size={12} />
  );
}

function renderServerSortIcon(
  sort: { column: string; descending: boolean } | null,
  columnName: string,
) {
  if (!sort || sort.column !== columnName) {
    return <ChevronsUpDown className="shrink-0 text-[var(--u-color-text-soft)]" size={12} />;
  }
  return sort.descending ? (
    <ArrowDown className="shrink-0 text-[var(--u-color-primary)]" size={12} />
  ) : (
    <ArrowUp className="shrink-0 text-[var(--u-color-primary)]" size={12} />
  );
}

function renderCell(value: string | null | undefined) {
  if (value === null || value === undefined) {
    return <StatusBadge>NULL</StatusBadge>;
  }

  if (value.length > 240) {
    return `${value.slice(0, 240)}...`;
  }

  return value;
}

function copyStatusLabel(
  status: "idle" | "copied-cell" | "copied-row" | "failed",
  t: ReturnType<typeof useI18n>["t"],
) {
  switch (status) {
    case "copied-cell":
      return t("database.grid.cellCopied");
    case "copied-row":
      return t("database.grid.rowCopied");
    case "failed":
      return t("database.grid.copyFailed");
    default:
      return t("database.grid.copyHint");
  }
}
