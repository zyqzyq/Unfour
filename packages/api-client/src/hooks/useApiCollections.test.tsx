// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiCollection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  createApiCollection: vi.fn(),
  deleteApiCollection: vi.fn(),
  listApiCollections: vi.fn(),
  renameApiCollection: vi.fn(),
}));

import {
  createApiCollection,
  deleteApiCollection,
  listApiCollections,
  renameApiCollection,
} from "@unfour/command-client";
import { useApiCollections } from "./useApiCollections";

const listMock = vi.mocked(listApiCollections);
const createMock = vi.mocked(createApiCollection);
const renameMock = vi.mocked(renameApiCollection);
const deleteMock = vi.mocked(deleteApiCollection);

function collection(overrides: Partial<ApiCollection> = {}): ApiCollection {
  return { id: "col-1", name: "Default", ...overrides } as ApiCollection;
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

describe("useApiCollections", () => {
  it("loads collections for the workspace", async () => {
    listMock.mockResolvedValue([collection({ id: "col-1" })]);

    const { result } = renderHook(() => useApiCollections("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.collections).toHaveLength(1);
    expect(listMock).toHaveBeenCalledWith("ws-1");
  });

  it("does not query while the workspace id is empty", () => {
    renderHook(() => useApiCollections(""), { wrapper: createWrapper() });
    expect(listMock).not.toHaveBeenCalled();
  });

  it("wires create and rename mutations to their commands", async () => {
    listMock.mockResolvedValue([]);
    createMock.mockResolvedValue(collection({ id: "c2" }));
    renameMock.mockResolvedValue(collection({ id: "c1", name: "Renamed" }));

    const { result } = renderHook(() => useApiCollections("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    result.current.createMut.mutate("New");
    result.current.renameMut.mutate({ id: "c1", name: "Renamed" });

    await waitFor(() => expect(createMock).toHaveBeenCalledWith("ws-1", "New"));
    await waitFor(() =>
      expect(renameMock).toHaveBeenCalledWith("ws-1", "c1", "Renamed"),
    );
  });

  it("refetches collections after deleting one", async () => {
    listMock.mockResolvedValue([collection({ id: "c1" })]);
    deleteMock.mockResolvedValue([]);

    const { result } = renderHook(() => useApiCollections("ws-1"), {
      wrapper: createWrapper(),
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(listMock).toHaveBeenCalledTimes(1);

    result.current.deleteMut.mutate("c1");

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("ws-1", "c1"));
    // delete invalidates the collections query, triggering a refetch.
    await waitFor(() => expect(listMock).toHaveBeenCalledTimes(2));
  });
});
