import { beforeEach, describe, expect, it, vi } from "vitest";
import { executeDatabaseScript } from "@unfour/command-client";
import { canConfirmBatch, createSqlBatch, executeSqlBatch, sqlAwaitingConfirmation } from "./run-sql-batch";
import type { DatabaseQueryWorkspaceTab } from "./types";
vi.mock("@unfour/command-client", () => ({ executeDatabaseScript: vi.fn() }));
const tab: DatabaseQueryWorkspaceTab = {
  id: "q1", kind: "query", title: "SQL", sql: "SELECT 1; DELETE FROM t;",
  catalog: null, schema: null, connectionId: "db1", error: null, result: null,
  results: [], activeResultIndex: 0, pendingConfirmation: false, resultTab: "results",
};
beforeEach(() => vi.clearAllMocks());

describe("SQL script command", () => {
  it("previews only the backend-selected current statement for confirmation", () => {
    const batch = createSqlBatch(tab, { cursorOffset: 15 }, "ws");
    expect(sqlAwaitingConfirmation(batch, { details: { statementRanges: [{ start: 10, end: tab.sql.length }] } })).toBe("DELETE FROM t;");
  });
  it("sends Run All as a single unmodified script without a cursor", async () => {
    const batch = createSqlBatch(tab, { mode: "all", sql: "DELETE FROM t;", cursorOffset: 15 }, "ws");
    await executeSqlBatch(batch, false);
    expect(executeDatabaseScript).toHaveBeenCalledTimes(1);
    expect(executeDatabaseScript).toHaveBeenCalledWith(expect.objectContaining({ sql: tab.sql, cursorOffset: undefined, confirmMutation: false }));
  });
  it("sends Run Selected SQL without a cursor and leaves dialect parsing to Rust", () => {
    const selected = "DO $$ BEGIN PERFORM ';'; END; $$; SELECT 2;";
    const batch = createSqlBatch(tab, { sql: selected, cursorOffset: 18 }, "ws");
    expect(batch.input.sql).toBe(selected);
    expect(batch.input.cursorOffset).toBeUndefined();
    expect(createSqlBatch(tab, { cursorOffset: 17 }, "ws").input.cursorOffset).toBe(17);
  });
  it("rejects confirmation control requests instead of treating them as a new run", () => {
    expect(() => createSqlBatch(tab, { resume: true }, "ws")).toThrow(/confirmation control requests/i);
    expect(() => createSqlBatch(tab, { cancelConfirmation: true }, "ws")).toThrow(/confirmation control requests/i);
  });
  it("confirms the whole original script and invalidates confirmation after edits or context changes", async () => {
    const batch = createSqlBatch(tab, { mode: "all" }, "ws");
    expect(canConfirmBatch(batch, tab, "ws")).toBe(true);
    expect(canConfirmBatch({ ...batch, input: { ...batch.input, catalog: undefined, schema: undefined } }, tab, "ws")).toBe(true);
    for (const patch of [{ sql: "DROP TABLE t" }, { connectionId: "db2" }, { schema: "other" }, { catalog: "other" }]) {
      expect(canConfirmBatch(batch, { ...tab, ...patch }, "ws")).toBe(false);
    }
    expect(canConfirmBatch(batch, tab, "other")).toBe(false);
    await executeSqlBatch(batch, true);
    expect(executeDatabaseScript).toHaveBeenCalledWith(expect.objectContaining({ sql: tab.sql, confirmMutation: true }));
  });
  it("preserves statement errors and skipped entries from one execution", async () => {
    const output = { stopped: false, statements: [{ index: 2, status: "failed", error: { code: "DATABASE_ERROR", message: "already exists" } }, { index: 3, status: "skipped" }] };
    vi.mocked(executeDatabaseScript).mockResolvedValueOnce(output as Awaited<ReturnType<typeof executeDatabaseScript>>);
    expect(await executeSqlBatch(createSqlBatch(tab, { mode: "all" }, "ws"), true)).toBe(output);
  });
});
