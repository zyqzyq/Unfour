import { ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { Button, EmptyState, IconButton, Select, Toolbar, ToolbarGroup } from "@unfour/ui";
import type { DatabaseTableViewState } from "../model/types";
import { TableDataGrid } from "./TableDataGrid";

export function TableDataTab({
  executePending,
  onPageChange,
  onRefresh,
  result,
  tableView,
}: {
  executePending: boolean;
  onPageChange: (pageIndex: number, pageSize: number) => void;
  onRefresh: () => void;
  result: DatabaseQueryResult | null;
  tableView: DatabaseTableViewState | null;
}) {
  if (!result || !tableView) {
    return <EmptyState className="m-2 min-h-0 flex-1">Open table preview from the connection tree or table structure.</EmptyState>;
  }

  const firstRow = tableView.totalRows === 0 ? 0 : tableView.pageIndex * tableView.pageSize + 1;
  const lastRow = Math.min(tableView.totalRows, (tableView.pageIndex + 1) * tableView.pageSize);
  const hasPrevious = tableView.pageIndex > 0;
  const hasNext = lastRow < tableView.totalRows;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar className="h-8">
        <ToolbarGroup className="min-w-0">
          <span className="truncate text-[12px] font-medium text-[var(--u-color-text)]">{tableView.tableName}</span>
          <span className="text-[11px] text-[var(--u-color-text-soft)]">
            {firstRow}-{lastRow} of {tableView.totalRows}
          </span>
        </ToolbarGroup>
        <ToolbarGroup>
          <Select
            aria-label="Table preview page size"
            className="w-[88px]"
            disabled={executePending}
            onChange={(event) => onPageChange(0, Number(event.target.value))}
            options={[
              { label: "50", value: "50" },
              { label: "100", value: "100" },
              { label: "250", value: "250" },
            ]}
            value={String(tableView.pageSize)}
          />
          <IconButton disabled={executePending} label="Refresh table preview" onClick={onRefresh}>
            <RefreshCw size={13} />
          </IconButton>
          <Button
            disabled={!hasPrevious || executePending}
            onClick={() => onPageChange(tableView.pageIndex - 1, tableView.pageSize)}
            size="sm"
            type="button"
            variant="outline"
          >
            <ChevronLeft size={13} />
            Prev
          </Button>
          <Button
            disabled={!hasNext || executePending}
            onClick={() => onPageChange(tableView.pageIndex + 1, tableView.pageSize)}
            size="sm"
            type="button"
            variant="outline"
          >
            Next
            <ChevronRight size={13} />
          </Button>
        </ToolbarGroup>
      </Toolbar>
      <TableDataGrid result={result} />
    </div>
  );
}
