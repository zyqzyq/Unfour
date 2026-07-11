import { ArrowDown, ArrowUp, ChevronsUpDown } from "lucide-react";
import type { DatabaseTableColumn } from "@unfour/command-client";
import {
  StatusBadge,
  type DataTableColumn,
  type useI18n,
} from "@unfour/ui";

export type SortState = { columnIndex: number; direction: "asc" | "desc" };

export function buildSkeletonRows(columns: DatabaseTableColumn[], count: number): Array<Array<string | null>> {
  return Array.from({ length: count }, () => columns.map(() => ""));
}

export function buildSkeletonColumns(
  columns: DatabaseTableColumn[],
  columnsWidths: Record<string, number>,
): DataTableColumn<Array<string | null>>[] {
  return [
    {
      header: "#",
      id: "__row_actions",
      width: columnsWidths["__row_actions"] ?? 48,
    },
    ...columns.map((column, columnIndex) => {
      const id = column.name || `column-${columnIndex}`;
      return {
        header: (
          <span className="truncate" title={column.name}>
            {column.name}
          </span>
        ),
        id,
        meta: column.dataType,
        width: columnsWidths[id] ?? Math.min(Math.max(column.name.length * 9 + 96, 140), 360),
        cell: () => <SkeletonCell />,
      } satisfies DataTableColumn<Array<string | null>>;
    }),
  ];
}

export function truncatePreview(value: string) {
  const text = value.length === 0 ? "''" : value;
  return text.length > 120 ? `${text.slice(0, 120)}…` : text;
}

export function compareCells(a: string | null, b: string | null) {
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

export function renderSortIcon(sort: SortState | null, columnIndex: number) {
  const active = sort && sort.columnIndex === columnIndex;
  return active ? (
    sort.direction === "asc" ? (
      <ArrowUp className="shrink-0 text-[var(--u-color-primary)]" size={12} />
    ) : (
      <ArrowDown className="shrink-0 text-[var(--u-color-primary)]" size={12} />
    )
  ) : (
    <ChevronsUpDown className="shrink-0 text-[var(--u-color-text-soft)] opacity-0 transition-opacity group-hover/header:opacity-100" size={12} />
  );
}

export function renderServerSortIcon(
  sort: { column: string; descending: boolean } | null,
  columnName: string,
) {
  const active = sort && sort.column === columnName;
  return active ? (
    sort.descending ? (
      <ArrowDown className="shrink-0 text-[var(--u-color-primary)]" size={12} />
    ) : (
      <ArrowUp className="shrink-0 text-[var(--u-color-primary)]" size={12} />
    )
  ) : (
    <ChevronsUpDown className="shrink-0 text-[var(--u-color-text-soft)] opacity-0 transition-opacity group-hover/header:opacity-100" size={12} />
  );
}

export function renderCell(value: string | null | undefined) {
  if (value === null || value === undefined) {
    return <StatusBadge>NULL</StatusBadge>;
  }

  if (value.length > 240) {
    return `${value.slice(0, 240)}...`;
  }

  return value;
}

export function copyStatusLabel(
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

export function SkeletonCell() {
  return (
    <div className="flex h-full items-center px-1">
      <div className="h-3 w-full animate-pulse rounded bg-[var(--u-color-border)]" />
    </div>
  );
}
