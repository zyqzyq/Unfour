// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
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

    expect(table).toHaveStyle({ minWidth: "200px" });
    expect(columns[0]).toHaveStyle({ width: "120px" });
    expect(columns[1]).toHaveStyle({ width: "80px" });
    expect(countHeader).toHaveClass("justify-end");
    expect(countCell).toHaveClass("text-right");
  });
});
