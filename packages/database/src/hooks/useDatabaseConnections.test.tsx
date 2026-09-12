// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { DatabaseConnection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  listDatabaseConnections: vi.fn(),
}));

import { listDatabaseConnections } from "@unfour/command-client";
import {
  databaseConnectionsQueryKey,
  replaceDatabaseConnectionInCache,
  resolveCachedDatabaseConnection,
  useDatabaseConnections,
} from "./useDatabaseConnections";

const listMock = vi.mocked(listDatabaseConnections);

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("databaseConnectionsQueryKey", () => {
  it("scopes the list cache to the workspace", () => {
    expect(databaseConnectionsQueryKey("ws-1")).toEqual(["database-connections", "ws-1"]);
  });
});

describe("replaceDatabaseConnectionInCache", () => {
  it("replaces the matching connection and appends unknown ids", () => {
    const current: DatabaseConnection[] = [
      { id: "db-a", credentialRef: "old-a" } as DatabaseConnection,
      { id: "db-b", credentialRef: "old-b" } as DatabaseConnection,
    ];
    expect(
      replaceDatabaseConnectionInCache(current, {
        id: "db-a",
        credentialRef: "new-a",
      } as DatabaseConnection),
    ).toEqual([
      { id: "db-a", credentialRef: "new-a" },
      { id: "db-b", credentialRef: "old-b" },
    ]);
    expect(
      replaceDatabaseConnectionInCache(undefined, {
        id: "db-c",
        credentialRef: "new-c",
      } as DatabaseConnection),
    ).toEqual([{ id: "db-c", credentialRef: "new-c" }]);
  });
});

describe("resolveCachedDatabaseConnection", () => {
  it("prefers the cached connection with the same id when reopening Edit", () => {
    const passed = { id: "db-a", credentialRef: "old-a" } as DatabaseConnection;
    const cached = [
      { id: "db-a", credentialRef: "new-a" } as DatabaseConnection,
      { id: "db-b", credentialRef: "old-b" } as DatabaseConnection,
    ];
    expect(resolveCachedDatabaseConnection(passed, cached).credentialRef).toBe("new-a");
    expect(resolveCachedDatabaseConnection(passed, undefined)).toBe(passed);
  });
});

describe("useDatabaseConnections", () => {
  it("loads connections for the workspace", async () => {
    listMock.mockResolvedValue([{ id: "conn-1" } as DatabaseConnection]);

    const { result } = renderHook(() => useDatabaseConnections("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.data).toHaveLength(1));
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("stays disabled while the workspace id is empty", () => {
    renderHook(() => useDatabaseConnections(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });

  it("stays disabled while the Database surface is inactive", () => {
    renderHook(() => useDatabaseConnections("ws-1", { active: false }), {
      wrapper: createWrapper(),
    });
    expect(listMock).not.toHaveBeenCalled();
  });
});
