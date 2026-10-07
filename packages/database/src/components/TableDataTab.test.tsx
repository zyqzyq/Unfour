// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DatabaseQueryResult, DatabaseTable } from "@unfour/command-client";
import type { TableEditing } from "../model/types";
import { TableDataTab } from "./TableDataTab";

afterEach(cleanup);

const table: DatabaseTable = {
  catalog: null,
  columns: [
    { dataType: "INTEGER", name: "id", nullable: false, primaryKey: true },
    { dataType: "JSON", name: "payload", nullable: true, primaryKey: false },
  ],
  kind: "table",
  name: "events",
  schema: null,
};

const result: DatabaseQueryResult = {
  affectedRows: 0,
  columns: table.columns.map((column) => ({ dataType: column.dataType, name: column.name })),
  durationMs: 1,
  rows: [["1", '{"status":"ready"}']],
  safety: { classification: "read", confirmed: true, message: null, requiresConfirmation: false },
};

const editing: TableEditing = {
  canInsert: true,
  canUpdateDelete: true,
  onApply: vi.fn(),
  onDeleteRow: vi.fn(),
  onInsertRow: vi.fn(),
  onRevert: vi.fn(),
  onUpdateCell: vi.fn(),
  pending: false,
  pendingChanges: [{
    id: "update:1",
    operation: "update",
    originalValues: [{ column: "payload", mode: "value", value: '{"status":"ready"}' }],
    primaryKey: [{ column: "id", mode: "value", value: "1" }],
    rowKey: "1",
    values: [{ column: "payload", mode: "value", value: '{"status":"changed"}' }],
  }],
  previewSql: "UPDATE events SET payload = ? WHERE id = ?",
  primaryKeyColumns: ["id"],
  rowKey: (row) => row[0] ?? "",
};

describe("TableDataTab", () => {
  it("uses lookahead without presenting a lower bound as an exact page count", () => {
    const onPageChange = vi.fn();
    render(<TableDataTab executePending={false} onPageChange={onPageChange} onRefresh={vi.fn()}
      onTableFilter={vi.fn()} onTableSort={vi.fn()} result={result} table={table}
      tableFilter="" tableSort={null} tableView={{ pageIndex: 0, pageSize: 1, readOnly: false,
        tableName: "events", totalRows: 2, totalRowsExact: false, hasMore: true }} />);
    expect(screen.getByText("Page 1")).toBeInTheDocument();
    expect(screen.queryByText("1-1 of 2")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next page" })).toBeEnabled();
  });
  it("keeps the table stable without rendering placeholder rows while loading", () => {
    const view = render(
      <TableDataTab
        executePending={false}
        loading
        onPageChange={vi.fn()}
        onRefresh={vi.fn()}
        onTableFilter={vi.fn()}
        onTableSort={vi.fn()}
        result={null}
        table={table}
        tableFilter=""
        tableSort={null}
        tableView={null}
      />,
    );

    const loadingTable = screen.getByRole("table");
    expect(loadingTable.querySelectorAll(".animate-pulse")).toHaveLength(0);

    view.rerender(
      <TableDataTab
        executePending={false}
        loading={false}
        onPageChange={vi.fn()}
        onRefresh={vi.fn()}
        onTableFilter={vi.fn()}
        onTableSort={vi.fn()}
        result={result}
        table={table}
        tableFilter=""
        tableSort={null}
        tableView={{ pageIndex: 0, pageSize: 50, readOnly: false, tableName: "events", totalRows: 1 }}
      />,
    );

    expect(screen.getByRole("table")).toBe(loadingTable);
  });

  it("keeps pending-change actions above the data grid", () => {
    render(
      <TableDataTab
        editing={editing}
        executePending={false}
        onPageChange={vi.fn()}
        onRefresh={vi.fn()}
        onTableFilter={vi.fn()}
        onTableSort={vi.fn()}
        result={result}
        table={table}
        tableFilter=""
        tableSort={null}
        tableView={{ pageIndex: 0, pageSize: 50, readOnly: false, tableName: "events", totalRows: 1 }}
      />,
    );

    const applyButton = screen.getByRole("button", { name: "Apply" });
    const grid = screen.getByRole("table");

    expect(applyButton.compareDocumentPosition(grid) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  });
});
