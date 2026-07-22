import { describe, expect, it, vi } from "vitest";
import type { DatabaseQueryResult } from "@unfour/command-client";

vi.mock("@unfour/command-client", () => ({
  executeDatabaseQuery: vi.fn(),
}));

import { executeDatabaseQuery } from "@unfour/command-client";
import { executeSqlBatch } from "./run-sql-batch";

const executeMock = vi.mocked(executeDatabaseQuery);

function result(patch: Partial<DatabaseQueryResult> = {}): DatabaseQueryResult {
  return {
    columns: [{ name: "n", dataType: "int" }],
    rows: [["1"]],
    affectedRows: 0,
    durationMs: 3,
    safety: { classification: "read", requiresConfirmation: false, confirmed: true, message: null },
    ...patch,
  };
}

describe("executeSqlBatch", () => {
  it("runs statements sequentially and collects results", async () => {
    executeMock.mockResolvedValueOnce(result()).mockResolvedValueOnce(result({ rows: [["2"]] }));
    const onStatementSuccess = vi.fn();
    const onSuccess = vi.fn();

    const outcome = await executeSqlBatch(
      {
        catalog: null,
        collected: [],
        connectionId: "conn-1",
        nextIndex: 0,
        schema: null,
        statements: ["select 1", "select 2"],
        tabId: "tab-1",
      },
      false,
      {
        cancelled: () => false,
        onConfirmationRequired: vi.fn(),
        onError: vi.fn(),
        onStatementSuccess,
        onSuccess,
        workspaceId: "ws-1",
      },
    );

    expect(outcome).toBe("completed");
    expect(executeMock).toHaveBeenCalledTimes(2);
    expect(onStatementSuccess).toHaveBeenCalledTimes(2);
    expect(onSuccess.mock.calls[0]?.[1]).toHaveLength(2);
  });

  it("pauses on CONFIRMATION_REQUIRED and resumes from the same statement", async () => {
    executeMock
      .mockResolvedValueOnce(result())
      .mockRejectedValueOnce({ code: "CONFIRMATION_REQUIRED" })
      .mockResolvedValueOnce(result({ affectedRows: 1, columns: [], rows: [] }));

    const onConfirmationRequired = vi.fn();
    const first = await executeSqlBatch(
      {
        catalog: null,
        collected: [],
        connectionId: "conn-1",
        nextIndex: 0,
        schema: null,
        statements: ["select 1", "delete from t"],
        tabId: "tab-1",
      },
      false,
      {
        cancelled: () => false,
        onConfirmationRequired,
        onError: vi.fn(),
        onStatementSuccess: vi.fn(),
        onSuccess: vi.fn(),
        workspaceId: "ws-1",
      },
    );

    expect(first).toBe("confirmation");
    const paused = onConfirmationRequired.mock.calls[0]?.[0];
    expect(paused.nextIndex).toBe(1);
    expect(paused.collected).toHaveLength(1);

    const onSuccess = vi.fn();
    const second = await executeSqlBatch(paused, true, {
      cancelled: () => false,
      onConfirmationRequired: vi.fn(),
      onError: vi.fn(),
      onStatementSuccess: vi.fn(),
      onSuccess,
      workspaceId: "ws-1",
    });

    expect(second).toBe("completed");
    expect(executeMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        sql: "delete from t",
        confirmMutation: true,
      }),
    );
    expect(onSuccess.mock.calls[0]?.[1]).toHaveLength(2);
  });
});
