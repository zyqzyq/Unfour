// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DatabaseQueryResult } from "@unfour/command-client";
import type { TableEditing } from "../model/types";
import { calculateAutoFitWidth } from "./table-data-grid-helpers";
import { TableDataGrid } from "./TableDataGrid";

vi.mock("./table-data-grid-helpers", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./table-data-grid-helpers")>();
  return {
    ...actual,
    calculateAutoFitWidth: vi.fn(actual.calculateAutoFitWidth),
  };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const result: DatabaseQueryResult = {
  affectedRows: 0,
  columns: [
    { dataType: "INTEGER", name: "id" },
    { dataType: "TEXT", name: "name" },
  ],
  durationMs: 1,
  rows: [["1", "Ada"]],
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
    originalValues: [{ column: "name", mode: "value", value: "Ada" }],
    primaryKey: [{ column: "id", mode: "value", value: "1" }],
    rowKey: "1",
    values: [{ column: "name", mode: "value", value: "Ada Lovelace" }],
  }],
  previewSql: "UPDATE users SET name = ? WHERE id = ?",
  primaryKeyColumns: ["id"],
  rowKey: (row) => row[0] ?? "",
};

describe("TableDataGrid", () => {
  it("copies selected data cells as TSV with full values and pending edits, skipping action columns", () => {
    const longValue = `first\t"${"x".repeat(260)}"\nlast`;
    render(<TableDataGrid editing={editing} result={{ ...result, rows: [["1", "Ada"], [null, longValue]] }} />);
    const rows = screen.getAllByRole<HTMLTableRowElement>("row");
    const lastCell = rows[2].cells[2];
    fireEvent.click(lastCell.querySelector("button")!);
    fireEvent.click(rows[1].cells[0], { shiftKey: true });
    const clipboardData = { setData: vi.fn() };
    expect(fireEvent.copy(lastCell, { clipboardData })).toBe(false);
    expect(clipboardData.setData).toHaveBeenCalledWith("text/plain", `1\tAda Lovelace\r\n\t"first\t""${"x".repeat(260)}""\nlast"`);
  });

  it("keeps native shortcut and copy behavior in an active cell editor", () => {
    render(<TableDataGrid editing={editing} result={result} />);
    const cell = screen.getByRole("button", { name: "Ada Lovelace" });
    fireEvent.click(cell);
    fireEvent.doubleClick(cell);
    const editor = screen.getByDisplayValue("Ada Lovelace");
    const clipboardData = { setData: vi.fn() };
    expect(fireEvent.keyDown(editor, { ctrlKey: true, key: "c" })).toBe(true);
    expect(fireEvent.copy(editor, { clipboardData })).toBe(true);
    expect(clipboardData.setData).not.toHaveBeenCalled();
  });

  it("does not recalculate auto-fit widths for interaction-only rerenders", () => {
    render(<TableDataGrid editing={editing} result={result} />);

    expect(calculateAutoFitWidth).toHaveBeenCalledTimes(result.columns.length);

    fireEvent.click(screen.getByRole("button", { name: "Ada Lovelace" }));

    expect(calculateAutoFitWidth).toHaveBeenCalledTimes(result.columns.length);
  });

  it("keeps the grid body neutral during an initial load instead of flashing fake rows", () => {
    const { container } = render(<TableDataGrid loading result={result} />);

    expect(screen.queryByText("Ada")).toBeNull();
    expect(container.querySelector(".animate-pulse")).toBeNull();
  });

  it("reserves room for the delete action before editing becomes available", () => {
    const view = render(<TableDataGrid loading result={result} />);

    expect(view.container.querySelector("col")?.getAttribute("style")).toContain("72px");

    view.rerender(<TableDataGrid editing={editing} result={result} />);

    expect(screen.getByRole("button", { name: /delete row/i })).toBeTruthy();
    expect(view.container.querySelector("col")?.getAttribute("style")).toContain("72px");
  });

  it("pages all returned rows locally, resets cell selections, and filters and sorts the whole batch", () => {
    const batch = {
      ...result,
      rows: Array.from({ length: 501 }, (_, i) => [String(i + 1), i === 500 ? "last match" : "example"]),
    };
    const view = render(<TableDataGrid result={batch} />);
    fireEvent.click(screen.getByText("1"));
    expect(screen.getByLabelText("Cell value")).toBeEnabled();
    fireEvent.click(screen.getByLabelText("Next page"));
    expect(screen.queryByLabelText("Cell value")).toBeNull();
    const clipboardData = { setData: vi.fn() };
    fireEvent.copy(screen.getByRole("table"), { clipboardData });
    expect(clipboardData.setData).not.toHaveBeenCalled();
    for (let page = 2; page < 6; page++) fireEvent.click(screen.getByLabelText("Next page"));
    expect(screen.getByText("501-501 of 501")).toBeVisible();
    expect(screen.getByText("last match")).toBeVisible();
    expect(screen.getByLabelText("Next page")).toBeDisabled();
    expect(screen.getByLabelText("Copy row 501")).toBeEnabled();

    fireEvent.change(screen.getByPlaceholderText("Filter rows"), { target: { value: "example" } });
    expect(screen.getByText("1-100 of 500")).toBeVisible();
    fireEvent.click(screen.getByTitle("Sort by id"));
    fireEvent.click(screen.getByTitle("Sort by id"));
    expect(view.container.querySelector("tbody tr")?.textContent).toContain("500");
    fireEvent.click(screen.getByLabelText("Next page"));
    view.rerender(<TableDataGrid result={result} />);
    expect(screen.getByPlaceholderText("Filter rows")).toHaveValue("");
    expect(screen.getByText("Ada")).toBeVisible();
    expect(screen.queryByLabelText("Next page")).toBeNull();
  });

  it("keeps duplicate query columns and an action-like alias independently resizable by ordinal", () => {
    const columns = ["value", "value", "__row_actions", ""].map((name) => ({ name, dataType: "TEXT" }));
    render(<TableDataGrid result={{ ...result, columns, rows: [["one", "two", "three", "four"]] }} />);
    const [first, second] = screen.getAllByRole("separator", { name: /Resize value;/ });
    const oldWidth = Number(first.getAttribute("aria-valuenow"));
    fireEvent.keyDown(first, { key: "ArrowRight" });
    expect(first).toHaveAttribute("aria-valuenow", String(oldWidth + 8));
    expect(second).toHaveAttribute("aria-valuenow", String(oldWidth));
    expect(screen.getByRole("separator", { name: /Resize __row_actions;/ })).toBeVisible();
    expect(screen.getByRole("separator", { name: /Resize #4;/ })).toBeVisible();
  });
});
