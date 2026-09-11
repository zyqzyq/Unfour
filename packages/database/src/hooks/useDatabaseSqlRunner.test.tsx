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

function setup() {
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
  act(() => { view.result.current.tabs.openQueryTab({ connectionId: "db", sql: "SELECT 1; CREATE TABLE t(n); SELECT 3;" }); });
  return { ...view, success, failure, connection };
}

describe("SQL runner outcomes", () => {
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
    act(() => result.current.tabs.updateQueryTab(result.current.tab!.id, { sql: "DROP TABLE t;" }));
    act(() => result.current.runner.runSql({ resume: true }));
    await waitFor(() => expect(execute).toHaveBeenCalledTimes(2));
    expect(execute.mock.calls[1][0].confirmMutation).toBe(false);
    await waitFor(() => expect(result.current.tab?.pendingConfirmation).toBe(true));
    act(() => result.current.runner.runSql({ cancelConfirmation: true }));
    expect(result.current.tab?.pendingConfirmation).toBe(false);
    expect(result.current.tab?.sql).toBe("DROP TABLE t;");
    expect(execute).toHaveBeenCalledTimes(2);
  });
});
