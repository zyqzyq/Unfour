// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { WorkspaceVariable } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  listWorkspaceVariables: vi.fn(),
  replaceWorkspaceVariables: vi.fn(),
}));

import {
  listWorkspaceVariables,
  replaceWorkspaceVariables,
} from "@unfour/command-client";
import { useWorkspaceVariables } from "./useWorkspaceVariables";

const listMock = vi.mocked(listWorkspaceVariables);
const replaceMock = vi.mocked(replaceWorkspaceVariables);

function workspaceVariable(
  overrides: Partial<WorkspaceVariable> = {},
): WorkspaceVariable {
  return {
    id: "var-1",
    workspaceId: "ws-1",
    key: "base_url",
    value: "https://workspace.example.com",
    isSecret: false,
    isEnabled: true,
    description: null,
    sortOrder: 0,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
    ...overrides,
  };
}

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

describe("useWorkspaceVariables", () => {
  it("writes replace results into the query cache without a list refetch", async () => {
    listMock.mockResolvedValue([]);
    const saved = [workspaceVariable()];
    replaceMock.mockResolvedValue(saved);

    const { result } = renderHook(() => useWorkspaceVariables("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await result.current.replaceMut.mutateAsync([
      {
        id: null,
        key: "base_url",
        value: "https://workspace.example.com",
        isSecret: false,
        isEnabled: true,
        description: null,
        sortOrder: 0,
      },
    ]);

    await waitFor(() => expect(result.current.variables).toEqual(saved));
    expect(listMock).toHaveBeenCalledTimes(1);
  });
});
