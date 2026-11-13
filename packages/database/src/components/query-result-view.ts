import type { DatabaseQueryResult } from "@unfour/command-client";
import { compareCells, type SortState } from "./table-data-grid-helpers";

type ResultRows = DatabaseQueryResult["rows"];

export type QueryResultView = {
  filter: string;
  sort: SortState | null;
  rows: ResultRows;
  onFilter: (value: string) => void;
  onSort: (value: SortState | null) => void;
};

/** Local view of one execution, in display order, before paging. */
export function filterQueryResultRows(rows: ResultRows, filter: string, sort: SortState | null): ResultRows {
  const needle = filter.trim().toLowerCase();
  const filtered = needle
    ? rows.filter((row) => row.some((value) => value?.toLowerCase().includes(needle)))
    : rows;
  if (!sort) return filtered;
  return [...filtered].sort((left, right) => {
    const compared = compareCells(left[sort.columnIndex], right[sort.columnIndex]);
    return sort.direction === "asc" ? compared : -compared;
  });
}
