import * as React from "react";
import { serializeDelimitedCell } from "./data-table-clipboard";
import { cn } from "./utils";

export type DataTableColumn<T> = {
  align?: "left" | "right";
  /** Target width in px when the resize handle is double-clicked. */
  autoFitWidth?: number;
  cell?: (row: T, rowIndex: number) => React.ReactNode;
  /** Raw clipboard value. Set false to omit an action column; by default copy the cell's visible text. */
  copyValue?: ((row: T, rowIndex: number) => string | null) | false;
  id: string;
  header: React.ReactNode;
  meta?: React.ReactNode;
  /** Initial column width in px. Set to enable column resize. */
  width?: number;
};

export type DataTableSelection = {
  /** End row index (inclusive). */
  endRow: number;
  /** End column index. */
  endCol: number;
  /** Start row index (inclusive). */
  startRow: number;
  /** Start column index. */
  startCol: number;
};

function isEditingCell(target: EventTarget) {
  return target instanceof HTMLElement
    && (target.isContentEditable || Boolean(target.closest("input,textarea,select,[role=combobox]")));
}

export function DataTable<T>({
  className,
  ref,
  columns,
  empty,
  getRowKey,
  rows,
  selection,
  getColumnResizeLabel,
  onSelectionChange,
  onColumnResize,
}: {
  className?: string;
  ref?: React.Ref<HTMLDivElement>;
  columns: DataTableColumn<T>[];
  empty?: React.ReactNode;
  getRowKey?: (row: T, rowIndex: number) => React.Key;
  rows: T[];
  /** Accessible drag/double-click hint for each column resize handle. */
  getColumnResizeLabel?: (column: DataTableColumn<T>) => string;
  /** Controlled cell selection range. */
  selection?: DataTableSelection | null;
  onSelectionChange?: (selection: DataTableSelection | null) => void;
  /** Called when a column resize handle is dragged. Pass new width in px. */
  onColumnResize?: (columnId: string, newWidth: number) => void;
}) {
  const tableRef = React.useRef<HTMLTableElement>(null);
  const resizingRef = React.useRef<{ colId: string; startX: number; startWidth: number } | null>(null);
  const [colWidths, setColWidths] = React.useState<Record<string, number>>(() => {
    const map: Record<string, number> = {};
    for (const c of columns) {
      if (c.width) map[c.id] = c.width;
    }
    return map;
  });
  // Fixed table width = sum of current column widths (initial or after resize).
  // The table does not stretch to fill its container; horizontal scrolling
  // kicks in via the outer `overflow-auto` wrapper when the container is
  // narrower than the summed column widths.
  const tableWidth = columns.reduce(
    (total, column) => total + (colWidths[column.id] ?? column.width ?? 0),
    0,
  );

  const isSelectable = !!onSelectionChange;

  const handleCellClick = React.useCallback(
    (rowIndex: number, colIndex: number, e: React.MouseEvent) => {
      if (!isSelectable) return;
      const sel: DataTableSelection = { startRow: rowIndex, endRow: rowIndex, startCol: colIndex, endCol: colIndex };
      onSelectionChange?.(e.shiftKey && selection ? { ...selection, endRow: rowIndex, endCol: colIndex } : sel);
    },
    [isSelectable, onSelectionChange, selection],
  );

  const handleKeyDown = React.useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (!isSelectable || !selection || e.defaultPrevented || isEditingCell(e.target)) return;
      const { startRow, endRow, startCol, endCol } = selection;
      let row = e.shiftKey ? endRow : startRow;
      let col = e.shiftKey ? endCol : startCol;

      switch (e.key) {
        case "ArrowUp": row = Math.max(0, row - 1); break;
        case "ArrowDown": row = Math.min(rows.length - 1, row + 1); break;
        case "ArrowLeft": col = Math.max(0, col - 1); break;
        case "ArrowRight": col = Math.min(columns.length - 1, col + 1); break;
        case "Escape": onSelectionChange?.(null); return;
        default: return;
      }
      e.preventDefault();
      e.currentTarget.ownerDocument.getSelection()?.removeAllRanges();
      onSelectionChange?.({
        startRow: e.shiftKey ? startRow : row,
        startCol: e.shiftKey ? startCol : col,
        endRow: row,
        endCol: col,
      });
    },
    [isSelectable, selection, columns, rows, onSelectionChange],
  );

  const handleResizeStart = React.useCallback(
    (colId: string, e: React.PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const currentWidth = colWidths[colId] ?? columns.find((column) => column.id === colId)?.width ?? 0;
      // Track the latest width in a local so `onUp` reads the post-drag value
      // instead of the stale `colWidths` captured in this closure.
      let latestWidth = currentWidth;
      resizingRef.current = { colId, startX: e.clientX, startWidth: currentWidth };

      function onMove(moveEvent: PointerEvent) {
        const ref = resizingRef.current;
        if (!ref) return;
        const delta = moveEvent.clientX - ref.startX;
        const newWidth = Math.max(40, ref.startWidth + delta);
        latestWidth = newWidth;
        setColWidths((prev) => ({ ...prev, [colId]: newWidth }));
      }

      function onUp() {
        if (resizingRef.current) {
          onColumnResize?.(colId, latestWidth);
        }
        resizingRef.current = null;
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      }

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp, { once: true });
    },
    [colWidths, columns, onColumnResize],
  );

  const resizeColumn = React.useCallback(
    (column: DataTableColumn<T>, width: number) => {
      const nextWidth = Math.max(40, Math.round(width));
      setColWidths((current) => ({ ...current, [column.id]: nextWidth }));
      onColumnResize?.(column.id, nextWidth);
    },
    [onColumnResize],
  );

  const handleResizeKeyDown = React.useCallback(
    (column: DataTableColumn<T>, event: React.KeyboardEvent) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      event.stopPropagation();
      const currentWidth = colWidths[column.id] ?? column.width ?? 40;
      const step = event.shiftKey ? 32 : 8;
      resizeColumn(column, currentWidth + (event.key === "ArrowRight" ? step : -step));
    },
    [colWidths, resizeColumn],
  );

  const isSelected = React.useCallback(
    (rowIndex: number, colIndex: number) => {
      if (!selection) return false;
      return rowIndex >= Math.min(selection.startRow, selection.endRow) && rowIndex <= Math.max(selection.startRow, selection.endRow)
        && colIndex >= Math.min(selection.startCol, selection.endCol) && colIndex <= Math.max(selection.startCol, selection.endCol);
    },
    [selection],
  );

  return (
    <div
      className={cn("min-h-0 overflow-auto", className)}
      ref={ref}
      onCopy={(event) => {
        if (!isSelectable || !selection || event.defaultPrevented || isEditingCell(event.target)) return;
        // A user can select individual characters in a read-only result cell.
        const textSelection = event.currentTarget.ownerDocument.getSelection();
        if (textSelection && !textSelection.isCollapsed) return;
        const text = selectedCellText(selection, columns, rows, tableRef.current);
        if (text === null) return;
        event.clipboardData.setData("text/plain", text);
        event.preventDefault();
      }}
      onKeyDown={handleKeyDown}
      tabIndex={isSelectable ? 0 : undefined}
    >
      <table
        className="table-fixed text-left text-[12px]"
        ref={tableRef}
        style={tableWidth ? { width: tableWidth } : undefined}
      >
        <colgroup>
          {columns.map((column) => {
            const width = colWidths[column.id] ?? column.width;
            return <col key={column.id} style={width ? { width } : undefined} />;
          })}
        </colgroup>
        <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
          <tr>
            {columns.map((column) => {
              const width = colWidths[column.id] ?? column.width;
              const resizeLabel = getColumnResizeLabel?.(column) ?? column.id;
              return (
                <th
                  className={cn(
                    "relative box-border h-[var(--u-size-table-row)] border-b border-r border-[var(--u-color-border)] px-3 font-semibold last:border-r-0",
                    column.align === "right" && "text-right",
                  )}
                  key={column.id}
                >
                  <div
                    className={cn(
                      "flex min-w-0 items-center gap-2",
                      column.align === "right" && "justify-end",
                    )}
                  >
                    <span className="truncate">{column.header}</span>
                    {column.meta && <span className="shrink-0 text-[length:var(--u-font-size-caption)] uppercase text-[var(--u-color-text-muted)]">{column.meta}</span>}
                  </div>
                  {onColumnResize && width && (
                    <div
                      aria-label={resizeLabel}
                      aria-orientation="vertical"
                      aria-valuemin={40}
                      aria-valuenow={Math.round(width)}
                      className="group absolute inset-y-0 right-0 z-10 w-2 cursor-col-resize focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-[var(--u-color-focus)]"
                      onDoubleClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        resizeColumn(column, column.autoFitWidth ?? column.width ?? width);
                      }}
                      onKeyDown={(event) => handleResizeKeyDown(column, event)}
                      onPointerDown={(event) => handleResizeStart(column.id, event)}
                      role="separator"
                      tabIndex={0}
                      title={resizeLabel}
                    >
                      <div className="absolute inset-y-1 -right-px w-px bg-[var(--u-color-primary)] opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-visible:opacity-100" />
                    </div>
                  )}
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                className="h-20 text-center text-[12px] text-[var(--u-color-text-muted)]"
                colSpan={Math.max(columns.length, 1)}
              >
                {empty ?? "No rows"}
              </td>
            </tr>
          ) : (
            rows.map((row, rowIndex) => (
              <tr
                className="border-b border-[color:color-mix(in_srgb,var(--u-color-border)_62%,transparent)] hover:bg-[var(--u-color-surface-hover)]"
                key={getRowKey?.(row, rowIndex) ?? rowIndex}
              >
                {columns.map((column, colIndex) => {
                  const content = column.cell?.(row, rowIndex);
                  return (
                    <td
                      className={cn(
                        "box-border h-[var(--u-size-table-row)] truncate border-r border-[var(--u-color-border)] px-3 text-[var(--u-color-text)] last:border-r-0",
                        column.align === "right" && "text-right",
                        isSelected(rowIndex, colIndex) && "bg-[color:color-mix(in_srgb,var(--u-color-primary)_12%,var(--u-color-surface))]",
                      )}
                      key={column.id}
                      onMouseDown={(event) => {
                        if (isSelectable && selection && event.shiftKey && event.button === 0 && !isEditingCell(event.target)) {
                          // Shift-click extends the grid rectangle, not the browser's text range.
                          event.preventDefault();
                          event.currentTarget.ownerDocument.getSelection()?.removeAllRanges();
                          tableRef.current?.parentElement?.focus({ preventScroll: true });
                        }
                      }}
                      onClick={(e) => handleCellClick(rowIndex, colIndex, e)}
                      title={typeof content === "string" || typeof content === "number" ? String(content) : undefined}
                    >
                      {content}
                    </td>
                  );
                })}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

function selectedCellText<T>(
  selection: DataTableSelection,
  columns: DataTableColumn<T>[],
  rows: T[],
  table: HTMLTableElement | null,
): string | null {
  const minRow = Math.max(0, Math.min(selection.startRow, selection.endRow));
  const maxRow = Math.min(rows.length - 1, Math.max(selection.startRow, selection.endRow));
  const minCol = Math.max(0, Math.min(selection.startCol, selection.endCol));
  const maxCol = Math.min(columns.length - 1, Math.max(selection.startCol, selection.endCol));
  const lines: string[] = [];
  for (let row = minRow; row <= maxRow; row++) {
    const cells: string[] = [];
    for (let col = minCol; col <= maxCol; col++) {
      const { copyValue } = columns[col];
      if (copyValue === false) continue;
      const value = copyValue ? copyValue(rows[row], row) : table?.tBodies[0]?.rows[row]?.cells[col]?.textContent;
      cells.push(serializeDelimitedCell(value, "\t"));
    }
    if (cells.length) lines.push(cells.join("\t"));
  }
  return lines.length ? lines.join("\r\n") : null;
}
