// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DataTable } from "./data-table";

afterEach(cleanup);

describe("DataTable", () => {
  it("keeps explicit column widths shared by header and body cells", () => {
    render(
      <DataTable
        columns={[
          { cell: (row) => row.name, header: "Name", id: "name", width: 120 },
          { align: "right", cell: (row) => row.count, header: "Count", id: "count", width: 80 },
        ]}
        rows={[{ count: 42, name: "Users" }]}
      />,
    );

    const table = screen.getByRole("table");
    const countHeader = screen.getByText("Count").closest("div");
    const countCell = screen.getByText("42").closest("td");
    const columns = table.querySelectorAll("col");

    expect(table).toHaveStyle({ width: "200px" });
    expect(columns[0]).toHaveStyle({ width: "120px" });
    expect(columns[1]).toHaveStyle({ width: "80px" });
    expect(countHeader).toHaveClass("justify-end");
    expect(countCell).toHaveClass("text-right");
  });

  it("renders a resize handle on each column header when onColumnResize is provided", () => {
    render(
      <DataTable
        columns={[
          { cell: (row) => row.name, header: "Name", id: "name", width: 120 },
          { cell: (row) => row.count, header: "Count", id: "count", width: 80 },
        ]}
        onColumnResize={() => {}}
        rows={[{ count: 42, name: "Users" }]}
      />,
    );

    // Each column header (<th>) holds a resize handle div with cursor-col-resize.
    const handles = document.querySelectorAll<HTMLElement>('.cursor-col-resize');
    expect(handles.length).toBe(2);
  });

  it("does not render resize handles when onColumnResize is absent", () => {
    render(
      <DataTable
        columns={[
          { cell: (row) => row.name, header: "Name", id: "name", width: 120 },
          { cell: (row) => row.count, header: "Count", id: "count", width: 80 },
        ]}
        rows={[{ count: 42, name: "Users" }]}
      />,
    );

    const handles = document.querySelectorAll('.cursor-col-resize');
    expect(handles.length).toBe(0);
  });

  it("reports the final width to onColumnResize after a drag", () => {
    const handleResize = vi.fn();
    render(
      <DataTable
        columns={[
          { cell: (row) => row.name, header: "Name", id: "name", width: 120 },
          { cell: (row) => row.count, header: "Count", id: "count", width: 80 },
        ]}
        onColumnResize={handleResize}
        rows={[{ count: 42, name: "Users" }]}
      />,
    );

    const handle = document.querySelector<HTMLElement>('.cursor-col-resize')!;
    fireEvent.pointerDown(handle, { clientX: 100 });

    // Simulate dragging 50px to the right.
    fireEvent.pointerMove(window, { clientX: 150 });
    fireEvent.pointerUp(window, { clientX: 150 });

    expect(handleResize).toHaveBeenCalledWith("name", 170);
  });
});
