// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { DatabaseStatementResult } from "@unfour/command-client";
import { QueryResultPanel } from "./QueryResultPanel";
afterEach(cleanup);

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
