// @vitest-environment jsdom
import type { ReactNode } from "react";
import type {
  WorkspaceEnvironment,
  WorkspaceEnvironmentVariable,
} from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  setActiveWorkspaceEnvironment: vi.fn(),
  createWorkspaceEnvironment: vi.fn(),
  deleteWorkspaceEnvironment: vi.fn(),
  listWorkspaceEnvironments: vi.fn(),
  updateWorkspaceEnvironmentVariables: vi.fn(),
}));

import {
  setActiveWorkspaceEnvironment,
  createWorkspaceEnvironment,
  deleteWorkspaceEnvironment,
  listWorkspaceEnvironments,
  updateWorkspaceEnvironmentVariables,
} from "@unfour/command-client";
import { useWorkspaceEnvironments } from "./useWorkspaceEnvironments";

const listMock = vi.mocked(listWorkspaceEnvironments);
const createMock = vi.mocked(createWorkspaceEnvironment);
const updateMock = vi.mocked(updateWorkspaceEnvironmentVariables);
const deleteMock = vi.mocked(deleteWorkspaceEnvironment);
const activateMock = vi.mocked(setActiveWorkspaceEnvironment);

function environmentVariable(
  overrides: Partial<WorkspaceEnvironmentVariable> = {},
): WorkspaceEnvironmentVariable {
  return {
    id: "var-1",
    workspaceId: "ws-1",
    environmentId: "env-2",
    key: "base_url",
    value: "https://qa.example.com",
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

function environment(overrides: Partial<WorkspaceEnvironment>): WorkspaceEnvironment {
  return {
    id: "env-1",
    workspaceId: "ws-1",
    name: "Local",
    sortOrder: 0,
    variables: [],
    isActive: false,
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

describe("useWorkspaceEnvironments", () => {
  it("loads environments and derives the active one", async () => {
    listMock.mockResolvedValue([
      environment({ id: "env-1" }),
      environment({ id: "env-2", isActive: true }),
    ]);
    const { result } = renderHook(() => useWorkspaceEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.activeEnvironment?.id).toBe("env-2");
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("does not query without a workspace id", () => {
    renderHook(() => useWorkspaceEnvironments(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });

  it("wires CRUD and activation through workspace commands", async () => {
    listMock.mockResolvedValue([environment({})]);
    createMock.mockResolvedValue(environment({ id: "env-2" }));
    deleteMock.mockResolvedValue([]);
    activateMock.mockResolvedValue([]);
    const { result } = renderHook(() => useWorkspaceEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    result.current.createMut.mutate("QA");
    result.current.deleteMut.mutate("env-1");
    result.current.activateMut.mutate("env-2");

    await waitFor(() => expect(createMock).toHaveBeenCalledWith("ws-1", "QA"));
    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("ws-1", "env-1"));
    await waitFor(() =>
      expect(activateMock).toHaveBeenCalledWith("ws-1", "env-2"),
    );
  });

  it("keeps variables after create then update without refetching the list", async () => {
    listMock.mockResolvedValue([environment({ id: "env-1", name: "Local" })]);
    const created = environment({ id: "env-2", name: "QA", variables: [] });
    const updated = environment({
      id: "env-2",
      name: "QA",
      updatedAt: "2026-01-02T00:00:00Z",
      variables: [environmentVariable()],
    });
    createMock.mockResolvedValue(created);
    updateMock.mockResolvedValue(updated);

    const { result } = renderHook(() => useWorkspaceEnvironments("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    await result.current.createMut.mutateAsync("QA");
    await result.current.updateMut.mutateAsync({
      id: "env-2",
      name: "QA",
      variables: [
        {
          id: null,
          key: "base_url",
          value: "https://qa.example.com",
          isSecret: false,
          isEnabled: true,
          description: null,
          sortOrder: 0,
        },
      ],
    });

    await waitFor(() => {
      const saved = result.current.environments.find((item) => item.id === "env-2");
      expect(saved?.variables).toEqual([environmentVariable()]);
    });
    expect(listMock).toHaveBeenCalledTimes(1);
  });
});
