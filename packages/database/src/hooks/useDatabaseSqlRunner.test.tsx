// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { executeDatabaseScript, stopDatabaseScript, type DatabaseScriptResult, type DatabaseStatementResult } from "@unfour/command-client";
import { resetDatabaseTabStore } from "../model/database-tab-state";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseSqlRunner } from "./useDatabaseSqlRunner";

vi.mock("@unfour/command-client", () => ({ executeDatabaseScript: vi.fn(), stopDatabaseScript: vi.fn() }));
const execute = vi.mocked(executeDatabaseScript);
const stop = vi.mocked(stopDatabaseScript);
beforeEach(() => { resetDatabaseTabStore(); vi.resetAllMocks(); });
afterEach(cleanup);

const FOUR_STATEMENT_SQL = `DROP TABLE IF EXISTS unfour_batch_test;
CREATE TABLE unfour_batch_test (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    score INT NOT NULL
);
INSERT INTO unfour_batch_test (name, score)
VALUES ('alpha', 10), ('beta', 20);
SELECT * FROM unfour_batch_test ORDER BY id;`;

function entry(index: number, status: DatabaseStatementResult["status"]): DatabaseStatementResult {
  return {
    index, status, start: 0, end: 9, sql: `SELECT ${index};`,
    error: status === "failed" ? { code: "DATABASE_ERROR", message: "already exists" } : null,
    result: status === "success" ? {
      columns: [{ name: "n", dataType: "int" }], rows: [[`${index}`]], affectedRows: 0, durationMs: 1,
      safety: { classification: "read", confirmed: true, requiresConfirmation: false, message: null },
    } : null,
  };
}

function renderRunner(workspaceId = "ws") {
  return renderHook(() => {
    const tabs = useDatabaseTabs({ workspaceId });
    const runner = useDatabaseSqlRunner({
      activeQueryTab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null,
      databaseTabs: tabs, workspaceId, t: (key) => key,
      browseMutation: { isPending: false, reset: vi.fn() }, browsingRef: { current: null },
      cancelledBrowseRequestsRef: { current: new WeakSet() },
      recordSuccessfulHistory: vi.fn(), recordFailedHistory: vi.fn(), setConnectionState: vi.fn(),
    });
    return { tabs, runner, tab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null };
  });
}

function setup(sql = "SELECT 1; CREATE TABLE t(n); SELECT 3;") {
  const success = vi.fn();
  const failure = vi.fn();
  const connection = vi.fn();
  const view = renderHook(() => {
    const tabs = useDatabaseTabs({ workspaceId: "ws" });
    const runner = useDatabaseSqlRunner({
      activeQueryTab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null,
      databaseTabs: tabs, workspaceId: "ws", t: (key) => key,
      browseMutation: { isPending: false, reset: vi.fn() }, browsingRef: { current: null },
      cancelledBrowseRequestsRef: { current: new WeakSet() },
      recordSuccessfulHistory: success, recordFailedHistory: failure, setConnectionState: connection,
    });
    return { tabs, runner, tab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null };
  });
  act(() => { view.result.current.tabs.openQueryTab({ connectionId: "db", sql }); });
  return { ...view, success, failure, connection };
}

describe("SQL runner outcomes", () => {
  it("sends the full editor SQL for Run All without a cursor", async () => {
    execute.mockResolvedValueOnce({ statements: [entry(1, "success")], stopped: false });
    const { result } = setup(FOUR_STATEMENT_SQL);
    act(() => result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(1));
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: FOUR_STATEMENT_SQL, cursorOffset: undefined, confirmMutation: false,
    }));
  });

  it("sends only the selection SQL for Run Selected", async () => {
    const selected = "DELETE FROM t;";
    execute.mockResolvedValueOnce({ statements: [entry(1, "success")], stopped: false });
    const { result } = setup(FOUR_STATEMENT_SQL);
    act(() => result.current.runner.runSql({ sql: selected }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(1));
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: selected, cursorOffset: undefined, confirmMutation: false,
    }));
  });

  it("does not execute when Run Selected receives empty SQL", () => {
    const { result } = setup(FOUR_STATEMENT_SQL);
    act(() => result.current.runner.runSql({ sql: "   " }));
    expect(execute).not.toHaveBeenCalled();
    expect(result.current.tab?.error).toEqual(expect.objectContaining({ code: "VALIDATION_ERROR" }));
  });

  it("still sends the full editor SQL for Run All when a selection payload is supplied", async () => {
    execute.mockResolvedValueOnce({ statements: [entry(1, "success")], stopped: false });
    const { result } = setup(FOUR_STATEMENT_SQL);
    act(() => result.current.runner.runSql({ mode: "all", sql: "DELETE FROM t;", cursorOffset: 15 }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(1));
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: FOUR_STATEMENT_SQL, cursorOffset: undefined, confirmMutation: false,
    }));
  });

  it("does not leave a late confirmation prompt after Stop", async () => {
    let reject!: (reason: unknown) => void;
    execute.mockReturnValueOnce(new Promise((_resolve, fail) => { reject = fail; }));
    stop.mockResolvedValue(true);
    const { result } = setup();
    act(() => result.current.runner.runSql({ mode: "all" }));
    act(() => result.current.runner.stopQuery());
    await act(async () => reject({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" }));
    expect(result.current.tab?.pendingConfirmation).toBe(false);
    expect(result.current.tab?.error).toBeNull();
    expect(result.current.tab?.executionNotice).toContain("database.batch.stopped");
  });
  it("keeps success results alongside a failed statement, records only executed history, and resets on rerun", async () => {
    const statements = [entry(1, "success"), entry(2, "failed"), entry(3, "skipped")];
    execute.mockResolvedValueOnce({ statements, stopped: false });
    const { result, success, failure } = setup();
    act(() => result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(result.current.tab?.statements).toHaveLength(3));
    expect(result.current.tab?.activeResultIndex).toBe(1);
    expect(result.current.tab?.error).toBeNull();
    expect(success).toHaveBeenCalledTimes(1);
    expect(failure).toHaveBeenCalledTimes(1);
    act(() => result.current.runner.selectQueryResult(0));
    expect(result.current.tab?.result?.rows).toEqual([["1"]]);
    execute.mockResolvedValueOnce({ statements: [entry(1, "failed"), entry(2, "skipped")], stopped: false });
    act(() => result.current.runner.runSql({ mode: "all" }));
    expect(result.current.tab?.statements).toEqual([]);
    await waitFor(() => expect(result.current.tab?.statements).toHaveLength(2));
    expect(result.current.tab?.results).toEqual([]);
  });

  it("Stop retains the real in-flight result and blocks duplicate or early reruns", async () => {
    let finish!: (value: DatabaseScriptResult) => void;
    execute.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    stop.mockResolvedValue(true);
    const { result, success } = setup();
    act(() => {
      result.current.runner.runSql({ mode: "all" });
      result.current.runner.runSql({ mode: "all" });
    });
    expect(execute).toHaveBeenCalledTimes(1);
    act(() => result.current.runner.stopQuery());
    await waitFor(() => expect(stop).toHaveBeenCalledTimes(1));
    expect(result.current.runner.sqlRunning).toBe(true);
    act(() => result.current.runner.runSql({ mode: "all" }));
    expect(execute).toHaveBeenCalledTimes(1);
    await act(async () => finish({ statements: [entry(1, "success"), entry(2, "skipped")], stopped: true }));
    expect(result.current.runner.sqlRunning).toBe(false);
    expect(result.current.tab?.result?.rows).toEqual([["1"]]);
    expect(success).toHaveBeenCalledTimes(1);
    expect(result.current.tab?.executionNotice).toContain("database.batch.stopped");
  });

  it("preflight confirmation produces no history and an edited script is never implicitly confirmed", async () => {
    execute.mockRejectedValue({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const { result, success, failure } = setup();
    act(() => result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    expect(success).not.toHaveBeenCalled();
    expect(failure).not.toHaveBeenCalled();
    expect(result.current.tab?.confirmationSql).toContain("CREATE TABLE");
    expect(result.current.tab?.error).toBeNull();
    act(() => result.current.tabs.updateQueryTab(result.current.tab!.id, { sql: "DROP TABLE t;" }));
    act(() => result.current.runner.runSql({ resume: true }));
    expect(execute).toHaveBeenCalledTimes(1);
    expect(result.current.tab?.pendingConfirmation).toBe(false);
    expect(result.current.tab?.executionNotice).toBe("database.batch.confirmationChanged");
    act(() => result.current.runner.runSql({ cancelConfirmation: true }));
    expect(result.current.tab?.sql).toBe("DROP TABLE t;");
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it("resumes the original Run All script after confirmation", async () => {
    execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const { result } = setup(FOUR_STATEMENT_SQL);
    act(() => result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    execute.mockResolvedValueOnce({ statements: [entry(1, "success"), entry(2, "success"), entry(3, "success"), entry(4, "success")], stopped: false });
    act(() => result.current.runner.runSql({ resume: true }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(2));
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: FOUR_STATEMENT_SQL, cursorOffset: undefined, confirmMutation: false,
    }));
    expect(execute.mock.calls[1][0]).toEqual(expect.objectContaining({
      sql: FOUR_STATEMENT_SQL, cursorOffset: undefined, confirmMutation: true,
    }));
  });

  it("does not execute when connection, schema, or catalog changes during confirmation", async () => {
    for (const patch of [{ connectionId: "other-db" }, { schema: "audit" }, { catalog: "other" }]) {
      execute.mockReset();
      execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
      const { result, unmount } = setup();
      act(() => result.current.runner.runSql({ mode: "all" }));
      await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
      act(() => result.current.tabs.updateQueryTab(result.current.tab!.id, patch));
      act(() => result.current.runner.runSql({ resume: true }));
      expect(execute).toHaveBeenCalledTimes(1);
      expect(result.current.tab?.pendingConfirmation).toBe(false);
      unmount();
      resetDatabaseTabStore();
    }
  });

  it("does not execute a stale confirmation after the workspace changes", async () => {
    execute.mockRejectedValue({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const view = renderHook(({ workspaceId }) => {
      const tabs = useDatabaseTabs({ workspaceId });
      const runner = useDatabaseSqlRunner({
        activeQueryTab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null,
        databaseTabs: tabs, workspaceId, t: (key) => key,
        browseMutation: { isPending: false, reset: vi.fn() }, browsingRef: { current: null },
        cancelledBrowseRequestsRef: { current: new WeakSet() },
        recordSuccessfulHistory: vi.fn(), recordFailedHistory: vi.fn(), setConnectionState: vi.fn(),
      });
      return { tabs, runner, tab: tabs.activeTab?.kind === "query" ? tabs.activeTab : null };
    }, { initialProps: { workspaceId: "ws" } });
    act(() => { view.result.current.tabs.openQueryTab({ connectionId: "db", sql: "DELETE FROM t;" }); });
    act(() => view.result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(view.result.current.tab?.pendingConfirmation).toBe(true));
    view.rerender({ workspaceId: "other" });
    act(() => view.result.current.runner.runSql({ resume: true }));
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it("resumes the original Run Selected SQL instead of the editor or current statement", async () => {
    const selected = "INSERT INTO t VALUES (1); DELETE FROM t;";
    execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const { result } = setup("SELECT 1; INSERT INTO t VALUES (1); DELETE FROM t; SELECT 2;");
    act(() => result.current.runner.runSql({ sql: selected }));
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    execute.mockResolvedValueOnce({ statements: [entry(1, "success"), entry(2, "success")], stopped: false });
    act(() => result.current.runner.runSql({ resume: true }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(2));
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: selected, cursorOffset: undefined, confirmMutation: false,
    }));
    expect(execute.mock.calls[1][0]).toEqual(expect.objectContaining({
      sql: selected, cursorOffset: undefined, confirmMutation: true,
    }));
  });

  it("does not fallback to another run mode after confirmation is invalidated", async () => {
    const selected = "DELETE FROM t;";
    execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const { result } = setup("SELECT 1; DELETE FROM t; SELECT 2;");
    act(() => result.current.runner.runSql({ sql: selected }));
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    act(() => result.current.tabs.updateQueryTab(result.current.tab!.id, { sql: "DROP TABLE t;" }));
    act(() => result.current.runner.runSql({ resume: true }));
    expect(execute).toHaveBeenCalledTimes(1);
    expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
      sql: selected, cursorOffset: undefined, confirmMutation: false,
    }));
    expect(result.current.tab?.pendingConfirmation).toBe(false);
    expect(result.current.tab?.executionNotice).toBe("database.batch.confirmationChanged");
  });

  it("resumes the original Explain current-statement cursor instead of offset 0", async () => {
    execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const { result } = setup("SELECT 1; DELETE FROM t; SELECT 2;");
    act(() => result.current.runner.runSql({ mode: "current", cursorOffset: 15 }));
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    execute.mockResolvedValueOnce({ statements: [entry(1, "success")], stopped: false });
    act(() => result.current.runner.runSql({ resume: true }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(2));
    expect(execute.mock.calls[0][0].cursorOffset).toBe(15);
    expect(execute.mock.calls[1][0]).toEqual(expect.objectContaining({
      sql: "SELECT 1; DELETE FROM t; SELECT 2;", cursorOffset: 15, confirmMutation: true,
    }));
  });

  it("restores a pending batch after the runner remounts", async () => {
    execute.mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED", message: "Confirm script" });
    const first = renderRunner();
    act(() => { first.result.current.tabs.openQueryTab({ connectionId: "db", sql: FOUR_STATEMENT_SQL }); });
    act(() => first.result.current.runner.runSql({ mode: "all" }));
    await waitFor(() => expect(first.result.current.tab?.pendingConfirmation).toBe(true));
    first.unmount();
    const second = renderRunner();
    expect(second.result.current.tab?.pendingConfirmation).toBe(true);
    execute.mockResolvedValueOnce({ statements: [entry(1, "success")], stopped: false });
    act(() => second.result.current.runner.runSql({ resume: true }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(2));
    expect(execute.mock.calls[1][0]).toEqual(expect.objectContaining({
      sql: FOUR_STATEMENT_SQL, cursorOffset: undefined, confirmMutation: true,
    }));
  });
});
