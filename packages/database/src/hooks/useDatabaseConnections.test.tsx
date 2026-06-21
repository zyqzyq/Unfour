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
import { useDatabaseConnections } from "./useDatabaseConnections";

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
});
