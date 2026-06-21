// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiEnvironment } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  activateApiEnvironment: vi.fn(),
  createApiEnvironment: vi.fn(),
  deleteApiEnvironment: vi.fn(),
  listApiEnvironments: vi.fn(),
  updateApiEnvironment: vi.fn(),
}));

import {
  activateApiEnvironment,
  createApiEnvironment,
  deleteApiEnvironment,
  listApiEnvironments,
} from "@unfour/command-client";
import { useApiEnvironments } from "./useApiEnvironments";

const listMock = vi.mocked(listApiEnvironments);
const createMock = vi.mocked(createApiEnvironment);
const deleteMock = vi.mocked(deleteApiEnvironment);
const activateMock = vi.mocked(activateApiEnvironment);

function environment(overrides: Partial<ApiEnvironment>): ApiEnvironment {
  return {
    id: "env-1",
    workspaceId: "ws-1",
    name: "Local",
    variables: [],
    isActive: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.resetAllMocks());

describe("useApiEnvironments", () => {
  it("loads environments and derives the active one", async () => {
    listMock.mockResolvedValue([
      environment({ id: "env-1", name: "Local" }),
      environment({ id: "env-2", name: "Staging", isActive: true }),
    ]);

    const { result } = renderHook(() => useApiEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.environments).toHaveLength(2);
    expect(result.current.activeEnvironment?.id).toBe("env-2");
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("returns a null active environment when none is active", async () => {
    listMock.mockResolvedValue([environment({ id: "env-1" })]);

    const { result } = renderHook(() => useApiEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.environments).toHaveLength(1));
    expect(result.current.activeEnvironment).toBeNull();
  });

  it("does not query while the workspace id is empty", () => {
    renderHook(() => useApiEnvironments(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });

  it("creates an environment and refetches the list on success", async () => {
    listMock.mockResolvedValue([environment({ id: "env-1" })]);
    createMock.mockResolvedValue(environment({ id: "env-2", name: "QA" }));

    const { result } = renderHook(() => useApiEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(listMock).toHaveBeenCalledTimes(1);

    result.current.createMut.mutate("QA");

    await waitFor(() => expect(createMock).toHaveBeenCalledWith("ws-1", "QA"));
    // Invalidation triggers a refetch of the active query.
    await waitFor(() => expect(listMock).toHaveBeenCalledTimes(2));
  });

  it("wires delete and activate mutations to their commands", async () => {
    listMock.mockResolvedValue([environment({ id: "env-1" })]);
    deleteMock.mockResolvedValue([]);
    activateMock.mockResolvedValue([]);

    const { result } = renderHook(() => useApiEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    result.current.deleteMut.mutate("env-1");
    result.current.activateMut.mutate("env-9");

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("ws-1", "env-1"));
    await waitFor(() =>
      expect(activateMock).toHaveBeenCalledWith("ws-1", "env-9"),
    );
  });
});
