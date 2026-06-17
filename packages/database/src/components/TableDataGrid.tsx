import { Clipboard } from "lucide-react";
import { useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { DataTable, IconButton, StatusBadge, type DataTableColumn } from "@unfour/ui";
import { serializeDatabaseCell, serializeDatabaseRow } from "../result-utils";

const MAX_RENDERED_ROWS = 500;

export function TableDataGrid({ result }: { result: DatabaseQueryResult }) {
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied-cell" | "copied-row" | "failed">("idle");
  const visibleRows = result.rows.slice(0, MAX_RENDERED_ROWS);

  async function copyText(text: string, status: "copied-cell" | "copied-row") {
    try {
      await navigator.clipboard.writeText(text);
      setCopyStatus(status);
      window.setTimeout(() => setCopyStatus("idle"), 1400);
    } catch {
      setCopyStatus("failed");
    }
  }

  const rowActionColumn: DataTableColumn<Array<string | null>> = {
    cell: (row, rowIndex) => (
      <IconButton
        label={`Copy row ${rowIndex + 1}`}
        onClick={() => copyText(serializeDatabaseRow(result, row, "\t"), "copied-row")}
        size="compact"
      >
        <Clipboard size={12} />
      </IconButton>
    ),
    header: "#",
    id: "__row_actions",
    width: 48,
  };

  const columns: DataTableColumn<Array<string | null>>[] = [
    rowActionColumn,
    ...result.columns.map((column, columnIndex) => ({
      cell: (row: Array<string | null>) => {
        const value = row[columnIndex];
        return (
          <button
            className="block w-full cursor-pointer truncate text-left font-mono text-[12px] text-[var(--u-color-text)] hover:text-[var(--u-color-primary)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]"
            onClick={() => copyText(serializeDatabaseCell(value, "\t"), "copied-cell")}
            title={value ?? "NULL"}
            type="button"
          >
            {renderCell(value)}
          </button>
        );
      },
      header: column.name,
      id: column.name || `column-${columnIndex}`,
      meta: column.dataType,
      width: Math.min(Math.max(column.name.length * 9 + 96, 140), 360),
    })),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <DataTable
        className="flex-1"
        columns={columns}
        empty="This result set is empty."
        getRowKey={(_, index) => index}
        rows={visibleRows}
      />
      <div className="flex h-7 shrink-0 items-center justify-between border-t border-[var(--u-color-border)] px-2 text-[11px] text-[var(--u-color-text-soft)]">
        <span>{copyStatusLabel(copyStatus)}</span>
        <span>
          {result.rows.length > visibleRows.length
            ? `Showing first ${visibleRows.length} of ${result.rows.length} rows`
            : `${result.rows.length} rows rendered`}
        </span>
      </div>
    </div>
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

function copyStatusLabel(status: "idle" | "copied-cell" | "copied-row" | "failed") {
  switch (status) {
    case "copied-cell":
      return "Cell copied";
    case "copied-row":
      return "Row copied";
    case "failed":
      return "Copy failed";
    default:
      return "Click a cell or row icon to copy";
  }
}
