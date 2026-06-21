import { ChevronLeft, ChevronRight, Plus, RefreshCw } from "lucide-react";
import { useState } from "react";
import type { DatabaseCellValue, DatabaseQueryResult } from "@unfour/command-client";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  EmptyState,
  IconButton,
  Input,
  Select,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import type { DatabaseTableViewState, TableEditing } from "../model/types";
import { TableDataGrid } from "./TableDataGrid";

export function TableDataTab({
  editing,
  executePending,
  onPageChange,
  onRefresh,
  result,
  tableView,
}: {
  editing?: TableEditing | null;
  executePending: boolean;
  onPageChange: (pageIndex: number, pageSize: number) => void;
  onRefresh: () => void;
  result: DatabaseQueryResult | null;
  tableView: DatabaseTableViewState | null;
}) {
  const { t } = useI18n();
  const [addOpen, setAddOpen] = useState(false);

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
          {editing ? (
            <Button
              disabled={editing.pending}
              onClick={() => setAddOpen(true)}
              size="sm"
              type="button"
              variant="outline"
            >
              <Plus size={13} />
              {t("database.editing.addRow")}
            </Button>
          ) : null}
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
      <TableDataGrid editing={editing} result={result} />
      {editing ? (
        <AddRowDialog
          columns={result.columns.map((column) => column.name)}
          onOpenChange={setAddOpen}
          onSubmit={(values) => {
            editing.onInsertRow(values);
            setAddOpen(false);
          }}
          open={addOpen}
          pending={editing.pending}
        />
      ) : null}
    </div>
  );
}

function AddRowDialog({
  columns,
  onOpenChange,
  onSubmit,
  open,
  pending,
}: {
  columns: string[];
  onOpenChange: (open: boolean) => void;
  onSubmit: (values: DatabaseCellValue[]) => void;
  open: boolean;
  pending: boolean;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<Record<string, string>>({});

  function submit() {
    // Empty inputs are sent as NULL; non-empty inputs as the typed text.
    const values: DatabaseCellValue[] = columns.map((column) => ({
      column,
      value: draft[column]?.length ? draft[column] : null,
    }));
    onSubmit(values);
    setDraft({});
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent title={t("database.editing.addRow")}>
        <DialogHeader>
          <DialogTitle>{t("database.editing.addRow")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="space-y-2">
          {columns.map((column) => (
            <label className="block space-y-1" key={column}>
              <span className="text-[11px] font-medium uppercase text-[var(--u-color-text-soft)]">{column}</span>
              <Input
                onChange={(event) => setDraft((current) => ({ ...current, [column]: event.target.value }))}
                placeholder={t("database.editing.nullPlaceholder")}
                value={draft[column] ?? ""}
              />
            </label>
          ))}
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">
            {t("common.confirm.cancel")}
          </Button>
          <Button disabled={pending} onClick={submit} size="sm" type="button">
            {t("database.editing.insert")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
