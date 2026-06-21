// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { DbQueryHistoryEntry } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { SqlHistoryEntry } from "../model/types";

vi.mock("@unfour/command-client", () => ({
  clearDatabaseQueryHistory: vi.fn(),
  listDatabaseQueryHistory: vi.fn(),
  recordDatabaseQueryHistory: vi.fn(),
}));

import {
  clearDatabaseQueryHistory,
  listDatabaseQueryHistory,
  recordDatabaseQueryHistory,
} from "@unfour/command-client";
import { dbQueryHistoryQueryKey, useQueryHistory } from "./useQueryHistory";

const listMock = vi.mocked(listDatabaseQueryHistory);
const recordMock = vi.mocked(recordDatabaseQueryHistory);
const clearMock = vi.mocked(clearDatabaseQueryHistory);

function persisted(overrides: Partial<DbQueryHistoryEntry> = {}): DbQueryHistoryEntry {
  return {
    id: "h1",
    workspaceId: "ws-1",
    connectionId: "conn-1",
    connectionName: "Local",
    sql: "select 1",
    status: "success",
    classification: null,
    rowCount: null,
    affectedRows: null,
    durationMs: null,
    error: null,
    executedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return {
    client,
    Wrapper: function Wrapper({ children }: { children: ReactNode }) {
      return (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      );
    },
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("dbQueryHistoryQueryKey", () => {
  it("builds a stable, workspace-scoped key", () => {
    expect(dbQueryHistoryQueryKey("ws-9")).toEqual(["db-query-history", "ws-9"]);
  });
});

describe("useQueryHistory", () => {
  it("maps nullable persisted fields into optional entry fields", async () => {
    listMock.mockResolvedValue([
      persisted({ rowCount: null, durationMs: 12, error: null }),
    ]);
    const { Wrapper } = createWrapper();

    const { result } = renderHook(() => useQueryHistory("ws-1", 50), {
      wrapper: Wrapper,
    });

    await waitFor(() => expect(result.current.entries).toHaveLength(1));
    const entry = result.current.entries[0];
    expect(entry.rowCount).toBeUndefined();
    expect(entry.error).toBeUndefined();
    expect(entry.durationMs).toBe(12);
    expect(listMock).toHaveBeenCalledWith("ws-1", 50);
  });

  it("records an entry, mapping optional fields back to null", async () => {
    listMock.mockResolvedValue([]);
    recordMock.mockResolvedValue(undefined);
    const { Wrapper } = createWrapper();

    const { result } = renderHook(() => useQueryHistory("ws-1", 50), {
      wrapper: Wrapper,
    });

    const entry: SqlHistoryEntry = {
      connectionId: "conn-1",
      connectionName: "Local",
      executedAt: "2026-01-01T00:00:00Z",
      id: "h2",
      sql: "select 2",
      status: "success",
    };
    result.current.record(entry);

    await waitFor(() => expect(recordMock).toHaveBeenCalledTimes(1));
    expect(recordMock).toHaveBeenCalledWith(
      expect.objectContaining({
        workspaceId: "ws-1",
        id: "h2",
        rowCount: null,
        error: null,
      }),
    );
  });

  it("optimistically empties the cache when clearing history", async () => {
    listMock.mockResolvedValue([persisted()]);
    clearMock.mockResolvedValue(undefined);
    const { Wrapper, client } = createWrapper();

    const { result } = renderHook(() => useQueryHistory("ws-1", 50), {
      wrapper: Wrapper,
    });
    await waitFor(() => expect(result.current.entries).toHaveLength(1));

    result.current.clear();

    await waitFor(() =>
      expect(client.getQueryData(dbQueryHistoryQueryKey("ws-1"))).toEqual([]),
    );
    expect(clearMock).toHaveBeenCalledWith("ws-1");
  });
});
