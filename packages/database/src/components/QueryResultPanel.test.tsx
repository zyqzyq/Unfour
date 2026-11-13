// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { DatabaseStatementResult } from "@unfour/command-client";
import { QueryResultPanel } from "./QueryResultPanel";
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("keeps transport errors in Messages and clears them after successful retry", () => {
  const { rerender } = render(<QueryResultPanel {...props} activeTab="messages" statements={[]} results={[]} error={{ code: "NETWORK_ERROR", message: "connection interrupted" }} />);
  expect(screen.getByText("connection interrupted")).toBeInTheDocument();
  expect(screen.queryByText("No messages")).not.toBeInTheDocument();
  rerender(<QueryResultPanel {...props} activeTab="messages" statements={[]} error={null} result={statements[0].result} />);
  expect(screen.queryByText("connection interrupted")).not.toBeInTheDocument();
});

it("keeps safety confirmation in Messages instead of presenting it as a failure", () => {
  render(<QueryResultPanel {...props} activeTab="messages" statements={[]} results={[]} pendingConfirmation confirmationSql="DELETE FROM test;" error={{ code: "CONFIRMATION_REQUIRED", message: "confirm" }} />);
  expect(screen.getByLabelText("Script awaiting confirmation")).toHaveTextContent("DELETE FROM test;");
  expect(screen.queryByText("Execution failed")).not.toBeInTheDocument();
});

const statements: DatabaseStatementResult[] = [
  { index: 1, start: 0, end: 9, sql: "SELECT 1;", status: "success", error: null, result: {
    columns: [{ name: "value", dataType: "int" }], rows: [["retained-value"]], affectedRows: 0, durationMs: 1,
    safety: { classification: "read", requiresConfirmation: false, confirmed: true, message: null },
  } },
  { index: 2, start: 9, end: 30, sql: "CREATE TABLE t(n);", status: "failed", result: null, error: { code: "DATABASE_ERROR", message: "table t already exists" } },
  { index: 3, start: 30, end: 39, sql: "SELECT 3;", status: "skipped", result: null, error: null },
];
const props: Parameters<typeof QueryResultPanel>[0] = {
  activeResultIndex: 1, activeTab: "results", error: null, history: [], isPending: false,
  onClearHistory: vi.fn(), onSelectHistory: vi.fn(), onSelectResultSet: vi.fn(), onSelectTab: vi.fn(),
  pendingConfirmation: false, result: null, results: [statements[0].result!], statements,
};
it("shows the failing SQL and lets an earlier successful result remain visible", () => {
  const { rerender } = render(<QueryResultPanel {...props} />);
  expect(screen.getByText("CREATE TABLE t(n);")).toBeInTheDocument();
  expect(screen.getAllByText(/already exists/).length).toBeGreaterThan(0);
  rerender(<QueryResultPanel {...props} activeResultIndex={0} result={statements[0].result} />);
  expect(screen.getByText("retained-value")).toBeInTheDocument();
  expect(screen.queryByText(/table t already exists/)).not.toBeInTheDocument();
});
it("includes failed and skipped SQL in messages and logs", () => {
  const { rerender } = render(<QueryResultPanel {...props} activeTab="messages" />);
  expect(screen.getByText("SELECT 3;")).toBeInTheDocument();
  expect(screen.getAllByText(/already exists/).length).toBeGreaterThan(0);
  rerender(<QueryResultPanel {...props} activeTab="logs" />);
  expect(screen.getByText("SELECT 3;")).toBeInTheDocument();
});
it("renders confirmation as a warning prompt instead of an execution failure", () => {
  const sql = "DROP TABLE t; CREATE TABLE t(n);";
  render(
    <QueryResultPanel
      {...props}
      confirmationSql={sql}
      error={{ code: "CONFIRMATION_REQUIRED", message: "This script may modify data", details: { statementCount: 2 } }}
      pendingConfirmation
      result={null}
      results={[]}
      statements={[]}
    />,
  );
  expect(screen.getAllByText("Confirmation required").length).toBeGreaterThan(0);
  expect(screen.getByText("Waiting for confirmation")).toBeInTheDocument();
  expect(screen.getByText(/safety check, not an execution failure/)).toBeInTheDocument();
  expect(screen.getByLabelText("Script awaiting confirmation")).toHaveTextContent(sql);
  expect(screen.queryByText("Failed")).not.toBeInTheDocument();
  expect(screen.queryByText("Execution failed")).not.toBeInTheDocument();
  expect(screen.queryByText("Technical detail")).not.toBeInTheDocument();
});
it("still renders DATABASE_ERROR as a failed execution", () => {
  render(
    <QueryResultPanel
      {...props}
      error={{ code: "DATABASE_ERROR", message: "syntax error at or near FROM" }}
      result={null}
      results={[]}
      statements={[]}
    />,
  );
  expect(screen.getByText("Failed")).toBeInTheDocument();
  expect(screen.getByText("Execution failed")).toBeInTheDocument();
  expect(screen.getByText("syntax error at or near FROM")).toBeInTheDocument();
});

const cappedResult = {
  columns: [{ name: "n", dataType: "int" }],
  rows: [["1"], ["2"]],
  affectedRows: 3,
  durationMs: 4,
  rowLimit: 2,
  safety: { classification: "mutation", requiresConfirmation: true, confirmed: true, message: null },
  truncated: false,
};

it("does not treat a result that exactly fills the display limit as truncated", () => {
  const { rerender } = render(
    <QueryResultPanel {...props} activeResultIndex={0} result={cappedResult} results={[cappedResult]} statements={[]} />,
  );
  expect(screen.queryByText("Truncated")).not.toBeInTheDocument();
  expect(screen.queryByText(/display limit/)).not.toBeInTheDocument();
  expect(screen.getByText("2 rows in 4ms")).toBeInTheDocument();
  rerender(
    <QueryResultPanel {...props} activeResultIndex={0} activeTab="logs" result={cappedResult} results={[cappedResult]} statements={[]} />,
  );
  expect(screen.getByText(/truncated=false rowLimit=2/)).toBeInTheDocument();
});

it("explains a real display-cap truncation separately from SQL LIMIT", () => {
  const result = { ...cappedResult, truncated: true };
  const statement = {
    index: 1, start: 0, end: 20, sql: "INSERT INTO t(n) VALUES (1),(2),(3) RETURNING n", status: "success" as const,
    error: null, result,
  };
  const { rerender } = render(
    <QueryResultPanel {...props} activeResultIndex={0} result={result} results={[result]} statements={[statement]} />,
  );
  expect(screen.getByText("Truncated")).toBeInTheDocument();
  expect(screen.getByText(/Returned 2 rows/)).toBeInTheDocument();
  expect(screen.getByText(/display limit of 2/)).toBeInTheDocument();
  expect(screen.getByText(/separate from any LIMIT/)).toBeInTheDocument();
  rerender(
    <QueryResultPanel {...props} activeResultIndex={0} activeTab="messages" result={result} results={[result]} statements={[statement]} />,
  );
  expect(screen.getAllByText(/separate from any LIMIT/).length).toBeGreaterThan(0);
});

it("copies and exports filtered rows in sorted order, or every returned row in original order", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator", Object.assign(Object.create(navigator), { clipboard: { writeText } }));
  const blobs: Blob[] = [];
  vi.stubGlobal("URL", class extends URL {
    static createObjectURL(blob: Blob) { blobs.push(blob); return "blob:query-result"; }
    static revokeObjectURL() {}
  });
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

  const result = {
    ...cappedResult, columns: [{ name: "id", dataType: "int" }, { name: "kind", dataType: "text" }],
    rows: [["2", "active"], ["0", "other"], ["1", "active"]],
  };
  const next = { ...cappedResult, rows: [["next result"]] };
  const panelProps = { ...props, statements: [], results: [result, next] };
  const panel = render(<QueryResultPanel {...panelProps} activeResultIndex={0} result={result} />);
  const search = screen.getByRole("textbox", { name: "Filter rows" });
  fireEvent.change(search, { target: { value: "active" } });
  fireEvent.click(screen.getByTitle("Sort by id"));
  expect(screen.getByText("Returned 3 rows · 2 match · 4ms")).toBeVisible();
  const scope = screen.getByRole("combobox", { name: "Copy and export scope" });
  expect(scope).toHaveValue("view");
  expect(screen.getByRole("option", { name: "Filtered rows (2)" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Copy TSV" }));
  await waitFor(() => expect(writeText).toHaveBeenLastCalledWith("id\tkind\r\n1\tactive\r\n2\tactive"));

  async function download(label: string) {
    fireEvent.keyDown(screen.getByRole("button", { name: "Export" }), { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("menuitem", { name: label }));
    const blob = blobs.at(-1)!;
    return new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(reader.error);
      reader.readAsText(blob);
    });
  }
  expect(await download("Export CSV")).toBe("id,kind\r\n1,active\r\n2,active");
  expect(JSON.parse(await download("Export JSON"))).toEqual([
    { id: "1", kind: "active" }, { id: "2", kind: "active" },
  ]);
  fireEvent.change(scope, { target: { value: "all" } });
  panel.rerender(<QueryResultPanel {...panelProps} activeResultIndex={0} activeTab="history" result={result} />);
  panel.rerender(<QueryResultPanel {...panelProps} activeResultIndex={0} activeTab="results" result={result} />);
  const returnedScope = screen.getByRole("combobox", { name: "Copy and export scope" });
  expect(returnedScope).toHaveValue("all");
  fireEvent.click(screen.getByRole("button", { name: "Copy TSV" }));
  await waitFor(() => expect(writeText).toHaveBeenLastCalledWith("id\tkind\r\n2\tactive\r\n0\tother\r\n1\tactive"));
  expect(JSON.parse(await download("Export JSON"))).toEqual([
    { id: "2", kind: "active" }, { id: "0", kind: "other" }, { id: "1", kind: "active" },
  ]);
  fireEvent.change(returnedScope, { target: { value: "view" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Filter rows" }), { target: { value: "missing" } });
  expect(await download("Export CSV")).toBe("id,kind");
  expect(JSON.parse(await download("Export JSON"))).toEqual([]);

  fireEvent.change(returnedScope, { target: { value: "all" } });
  panel.rerender(<QueryResultPanel {...panelProps} activeResultIndex={1} result={next} />);
  expect(screen.getByRole("textbox", { name: "Filter rows" })).toHaveValue("");
  expect(screen.queryByRole("combobox", { name: "Copy and export scope" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "Filter rows" }), { target: { value: "next" } });
  expect(screen.getByRole("combobox", { name: "Copy and export scope" })).toHaveValue("view");
  expect(await download("Export CSV")).toBe("n\r\nnext result");
});

it("disables output and removes old row counts when an error hides a retained result", () => {
  const result = { ...cappedResult, truncated: true };
  render(<QueryResultPanel {...props} activeResultIndex={0} result={result} results={[result]}
    statements={[]} error={{ code: "VALIDATION_ERROR", message: "Select a connection before running" }} />);
  expect(screen.getByText("Select a connection before running")).toBeVisible();
  expect(screen.getByRole("button", { name: "Copy TSV" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Export" })).toBeDisabled();
  expect(screen.queryByText("2 rows in 4ms")).not.toBeInTheDocument();
  expect(screen.queryByText("Truncated")).not.toBeInTheDocument();
});
