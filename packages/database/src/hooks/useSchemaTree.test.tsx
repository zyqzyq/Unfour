// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { DatabaseConnection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  getDatabaseSchema: vi.fn(),
}));

import { getDatabaseSchema } from "@unfour/command-client";
import { useSchemaTree } from "./useSchemaTree";

const schemaMock = vi.mocked(getDatabaseSchema);
const connection = { id: "conn-1" } as unknown as DatabaseConnection;

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

describe("useSchemaTree", () => {
  it("fetches the schema when every requirement is satisfied", async () => {
    schemaMock.mockResolvedValue({ tables: [] } as never);

    const { result } = renderHook(
      () =>
        useSchemaTree({
          connection,
          connectionId: "conn-1",
          workspaceId: "ws-1",
        }),
      { wrapper: createWrapper() },
    );

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(schemaMock).toHaveBeenCalledWith("ws-1", "conn-1");
  });

  it.each([
    ["no connection", { connection: null, connectionId: "conn-1", workspaceId: "ws-1" }],
    ["no connectionId", { connection, connectionId: null, workspaceId: "ws-1" }],
    ["no workspaceId", { connection, connectionId: "conn-1", workspaceId: "" }],
    [
      "explicitly disabled",
      { connection, connectionId: "conn-1", enabled: false, workspaceId: "ws-1" },
    ],
  ])("stays disabled when %s", (_label, params) => {
    renderHook(() => useSchemaTree(params), { wrapper: createWrapper() });
    expect(schemaMock).not.toHaveBeenCalled();
  });
});
