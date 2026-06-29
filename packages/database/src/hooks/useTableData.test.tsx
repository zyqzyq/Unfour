// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { DatabaseBrowseResult } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  browseDatabaseTable: vi.fn(),
}));

import { browseDatabaseTable } from "@unfour/command-client";
import { useTableData } from "./useTableData";

const browseMock = vi.mocked(browseDatabaseTable);

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("useTableData", () => {
  it("translates page index and size into a limit/offset browse request", async () => {
    const browseResult = { tableName: "users" } as unknown as DatabaseBrowseResult;
    browseMock.mockResolvedValue(browseResult);
    const onBrowseStart = vi.fn();
    const onSuccess = vi.fn();

    const { result } = renderHook(
      () =>
        useTableData({
          onBrowseStart,
          onSuccess,
          workspaceId: "ws-1",
        }),
      { wrapper: createWrapper() },
    );

    result.current.mutate({
      connectionId: "conn-1",
      pageIndex: 2,
      pageSize: 25,
      schema: "public",
      tableName: "users",
    });

    await waitFor(() => expect(onSuccess).toHaveBeenCalled());
    expect(onSuccess.mock.calls[0][0]).toBe(browseResult);
    expect(onBrowseStart).toHaveBeenCalled();
    expect(browseMock).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      connectionId: "conn-1",
      schema: "public",
      tableName: "users",
      limit: 25,
      offset: 50,
      orderBy: null,
      orderDescending: false,
      filter: null,
    });
  });

  it("browses the connection passed in the mutate call", async () => {
    browseMock.mockResolvedValue({} as DatabaseBrowseResult);

    const { result } = renderHook(
      () =>
        useTableData({
          onBrowseStart: vi.fn(),
          onSuccess: vi.fn(),
          workspaceId: "ws-1",
        }),
      { wrapper: createWrapper() },
    );

    result.current.mutate({ connectionId: "conn-2", pageIndex: 0, pageSize: 10, tableName: "logs" });

    await waitFor(() => expect(browseMock).toHaveBeenCalled());
    expect(browseMock).toHaveBeenCalledWith(
      expect.objectContaining({ connectionId: "conn-2", offset: 0, limit: 10 }),
    );
  });
});
