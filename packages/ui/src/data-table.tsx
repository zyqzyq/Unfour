import * as React from "react";
import { cn } from "./utils";

export type DataTableColumn<T> = {
  align?: "left" | "right";
  cell?: (row: T, rowIndex: number) => React.ReactNode;
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

function normalizeSelection(a: DataTableSelection, b: DataTableSelection): DataTableSelection {
  return {
    startRow: Math.min(a.startRow, b.startRow),
    endRow: Math.max(a.startRow, b.endRow),
    startCol: Math.min(a.startCol, b.startCol),
    endCol: Math.max(a.endCol, b.endCol),
  };
}

export function DataTable<T>({
  className,
  columns,
  empty,
  getRowKey,
  rows,
  selection,
  onSelectionChange,
  onColumnResize,
}: {
  className?: string;
  columns: DataTableColumn<T>[];
  empty?: React.ReactNode;
  getRowKey?: (row: T, rowIndex: number) => React.Key;
  rows: T[];
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
      onSelectionChange?.(e.shiftKey && selection ? normalizeSelection(selection, sel) : sel);
    },
    [isSelectable, onSelectionChange, selection],
  );

  const handleKeyDown = React.useCallback(
    (e: React.KeyboardEvent) => {
      if (!isSelectable || !selection) return;
      const { startRow, endRow, startCol, endCol } = selection;
      const row = e.shiftKey ? endRow : startRow;
      const col = e.shiftKey ? endCol : startCol;
      let next: DataTableSelection | undefined;

      switch (e.key) {
        case "ArrowUp": next = { ...selection, endRow: Math.max(0, row - 1) }; break;
        case "ArrowDown": next = { ...selection, endRow: Math.min(rows.length - 1, row + 1) }; break;
        case "ArrowLeft": next = { ...selection, endCol: Math.max(0, col - 1) }; break;
        case "ArrowRight": next = { ...selection, endCol: Math.min(columns.length - 1, col + 1) }; break;
        case "Escape": onSelectionChange?.(null); return;
        case "c":
        case "C":
          if (e.ctrlKey || e.metaKey) {
            e.preventDefault();
            copySelectionToClipboard(selection, columns, rows, tableRef.current);
          }
          return;
        default: return;
      }
      if (next) {
        e.preventDefault();
        onSelectionChange?.(e.shiftKey ? normalizeSelection(selection, next) : { startRow: next.endRow, endRow: next.endRow, startCol: next.endCol, endCol: next.endCol });
      }
    },
    [isSelectable, selection, columns, rows, onSelectionChange],
  );

  const handleResizeStart = React.useCallback(
    (colId: string, e: React.PointerEvent) => {
      const currentWidth = colWidths[colId] ?? 0;
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
    [colWidths, onColumnResize],
  );

  const isSelected = React.useCallback(
    (rowIndex: number, colIndex: number) => {
      if (!selection) return false;
      const n = selection.startRow <= selection.endRow
        ? selection
        : { startRow: selection.endRow, endRow: selection.startRow, startCol: selection.startCol, endCol: selection.endCol };
      const colNorm = n.startCol <= n.endCol ? n : { ...n, startCol: n.endCol, endCol: n.startCol };
      return rowIndex >= n.startRow && rowIndex <= n.endRow && colIndex >= colNorm.startCol && colIndex <= colNorm.endCol;
    },
    [selection],
  );

  return (
    <div
      className={cn("min-h-0 overflow-auto", className)}
      onKeyDown={handleKeyDown}
      tabIndex={isSelectable ? 0 : undefined}
    >
      <table
        className="table-fixed text-left text-[12px]"
        ref={tableRef}
        style={tableWidth ? { width: tableWidth } : undefined}
      >
        <colgroup>
          {columns.map((column) => (
            <col key={column.id} style={colWidths[column.id] ? { width: colWidths[column.id] } : undefined} />
          ))}
        </colgroup>
        <thead className="sticky top-0 z-10 bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text-muted)]">
          <tr>
            {columns.map((column) => (
              <th
                className={cn(
                  "relative box-border h-[var(--u-size-table-row)] border-b border-[var(--u-color-border)] px-2 font-medium",
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
                  {column.meta && <span className="shrink-0 text-[10px] uppercase text-[var(--u-color-text-soft)]">{column.meta}</span>}
                </div>
                {onColumnResize && colWidths[column.id] && (
                  <div
                    className="group absolute inset-y-0 right-0 z-10 w-2 cursor-col-resize"
                    onPointerDown={(e) => handleResizeStart(column.id, e)}
                  >
                    <div className="absolute inset-y-2 right-0 w-px bg-[var(--u-color-primary)] opacity-0 transition-opacity duration-100 group-hover:opacity-100" />
                  </div>
                )}
              </th>
            ))}
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
                {columns.map((column, colIndex) => (
                  <td
                    className={cn(
                      "box-border h-[var(--u-size-table-row)] truncate px-2 text-[var(--u-color-text)]",
                      column.align === "right" && "text-right",
                      isSelected(rowIndex, colIndex) && "bg-[color:color-mix(in_srgb,var(--u-color-primary)_12%,var(--u-color-surface))]",
                    )}
                    key={column.id}
                    onClick={(e) => handleCellClick(rowIndex, colIndex, e)}
                    title={String(column.cell?.(row, rowIndex) ?? "")}
                  >
                    {column.cell ? column.cell(row, rowIndex) : null}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}

/** Copy selected cells as tab-separated values to clipboard. */
function copySelectionToClipboard<T>(
  selection: DataTableSelection,
  columns: DataTableColumn<T>[],
  rows: T[],
  tableEl: HTMLTableElement | null,
) {
  const minRow = Math.min(selection.startRow, selection.endRow);
  const maxRow = Math.max(selection.startRow, selection.endRow);
  const minCol = Math.min(selection.startCol, selection.endCol);
  const maxCol = Math.max(selection.startCol, selection.endCol);

  const lines: string[] = [];
  for (let r = minRow; r <= maxRow; r++) {
    const cells: string[] = [];
    for (let c = minCol; c <= maxCol; c++) {
      const col = columns[c];
      const row = rows[r];
      const text = col?.cell ? String(col.cell(row, r) ?? "") : "";
      cells.push(text.includes("\t") ? `"${text}"` : text);
    }
    lines.push(cells.join("\t"));
  }

  navigator.clipboard?.writeText(lines.join("\n")).catch(() => {
    // Fallback: select the table cells so the browser can copy
    const sel = window.getSelection();
    if (!sel || !tableEl) return;
    // Try to use the table cells as a selection range
    const range = document.createRange();
    try {
      const startCell = tableEl.rows[minRow + 1]?.cells[minCol];
      const endCell = tableEl.rows[maxRow + 1]?.cells[maxCol];
      if (startCell && endCell) {
        range.setStart(startCell, 0);
        range.setEnd(endCell, 0);
        sel.removeAllRanges();
        sel.addRange(range);
      }
    } catch {
      // ignore
    }
  });
}
