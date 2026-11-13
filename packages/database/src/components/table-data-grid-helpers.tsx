import { ArrowDown, ArrowUp, ChevronsUpDown } from "lucide-react";
import type { DatabaseTableColumn } from "@unfour/command-client";
import {
  StatusBadge,
  type useI18n,
} from "@unfour/ui";

export type SortState = { columnIndex: number; direction: "asc" | "desc" };

export const MAX_CELL_PREVIEW_LENGTH = 240;
export const MAX_CELL_TITLE_LENGTH = 512;
export const MAX_VALUE_VIEWER_LENGTH = 20_000;
const MIN_AUTO_FIT_COLUMN_WIDTH = 96;
const MAX_AUTO_FIT_COLUMN_WIDTH = 560;

export function truncateText(value: string, maxLength: number) {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, Math.max(0, maxLength - 1))}…`;
}

export function truncatePreview(value: string) {
  return truncateText(value.length === 0 ? "''" : value, MAX_CELL_PREVIEW_LENGTH);
}

export function calculateAutoFitWidth(
  column: Pick<DatabaseTableColumn, "dataType" | "name">,
  values: Array<string | null | undefined>,
) {
  const headerWidth = estimateTextWidth(column.name) + estimateTextWidth(column.dataType, 6) + 32;
  const contentWidth = values.reduce(
    (maximum, value) => Math.max(maximum, estimateTextWidth(value == null ? "NULL" : truncatePreview(value)) + 20),
    MIN_AUTO_FIT_COLUMN_WIDTH,
  );
  return Math.min(MAX_AUTO_FIT_COLUMN_WIDTH, Math.max(MIN_AUTO_FIT_COLUMN_WIDTH, headerWidth, contentWidth));
}

export function isLikelyJson(value: string) {
  const trimmed = value.trim();
  return (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"));
}

function estimateTextWidth(value: string, pixelsPerUnit = 7.2) {
  const units = Array.from(value).reduce((total, character) => {
    if (character === "\t") return total + 4;
    return total + (character.charCodeAt(0) > 255 ? 2 : 1);
  }, 0);
  return Math.ceil(units * pixelsPerUnit);
}

export function compareCells(a: string | null, b: string | null, direction: SortState["direction"]) {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  const order = direction === "asc" ? 1 : -1;
  const numA = Number(a);
  const numB = Number(b);
  if (!Number.isNaN(numA) && !Number.isNaN(numB) && a.trim() !== "" && b.trim() !== "") {
    if (numA !== numB) return (numA < numB ? -1 : 1) * order;
    // Drivers return BIGINT values as strings. Adjacent IDs can round to the same Number.
    if (!Number.isSafeInteger(numA) && /^[+-]?\d+$/.test(a) && /^[+-]?\d+$/.test(b)) {
      const left = BigInt(a);
      const right = BigInt(b);
      if (left !== right) return (left < right ? -1 : 1) * order;
    }
    return 0;
  }
  return a.localeCompare(b) * order;
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

  return truncatePreview(value);
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
