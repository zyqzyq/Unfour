import { describe, expect, it } from "vitest";
import { filterQueryResultRows } from "./query-result-view";
import {
  calculateAutoFitWidth,
  isLikelyJson,
  MAX_CELL_PREVIEW_LENGTH,
  truncatePreview,
  truncateText,
} from "./table-data-grid-helpers";

describe("table data grid display limits", () => {
  it("orders returned 64-bit IDs exactly and leaves NULL last in either direction", () => {
    const rows = [["9007199254740993"], [null], ["-9007199254740992"], ["9007199254740992"], ["-9007199254740993"]];
    const sorted = (direction: "asc" | "desc") =>
      filterQueryResultRows(rows, "", { columnIndex: 0, direction }).map(([id]) => id);
    const ascending = ["-9007199254740993", "-9007199254740992", "9007199254740992", "9007199254740993"];
    expect(sorted("asc")).toEqual([...ascending, null]);
    expect(sorted("desc")).toEqual([...ascending.reverse(), null]);
    expect(rows[0][0]).toBe("9007199254740993");
  });

  it("caps the rendered cell preview at the configured maximum", () => {
    const preview = truncatePreview("x".repeat(MAX_CELL_PREVIEW_LENGTH + 20));

    expect(preview).toHaveLength(MAX_CELL_PREVIEW_LENGTH);
    expect(preview.endsWith("…")).toBe(true);
  });

  it("keeps generic truncation within the requested maximum", () => {
    expect(truncateText("123456", 4)).toBe("123…");
    expect(truncateText("123", 4)).toBe("123");
  });

  it("caps auto-fit width when a cell contains very long content", () => {
    expect(calculateAutoFitWidth(
      { dataType: "JSON", name: "payload" },
      [JSON.stringify({ content: "x".repeat(5_000) })],
    )).toBe(560);
  });

  it("detects object and array shaped JSON without parsing oversized values", () => {
    expect(isLikelyJson('{"name":"Ada"}')).toBe(true);
    expect(isLikelyJson("[1,2,3]")).toBe(true);
    expect(isLikelyJson("plain text")).toBe(false);
  });
});
