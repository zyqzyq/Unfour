// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiCollectionFolder } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";

vi.mock("@unfour/command-client", () => ({
  createApiCollectionFolder: vi.fn(),
  deleteApiCollectionFolder: vi.fn(),
  listApiCollectionFolders: vi.fn(),
  moveApiCollectionFolder: vi.fn(),
  moveApiRequest: vi.fn(),
  renameApiCollectionFolder: vi.fn(),
  reorderApiCollectionFolders: vi.fn(),
  reorderApiRequests: vi.fn(),
}));

import {
  createApiCollectionFolder,
  listApiCollectionFolders,
  moveApiRequest,
  renameApiCollectionFolder,
} from "@unfour/command-client";
import { useApiCollectionFolders } from "./useApiCollectionFolders";

const listMock = vi.mocked(listApiCollectionFolders);
const createMock = vi.mocked(createApiCollectionFolder);
const renameMock = vi.mocked(renameApiCollectionFolder);
const moveRequestMock = vi.mocked(moveApiRequest);

function folder(overrides: Partial<ApiCollectionFolder> = {}): ApiCollectionFolder {
  return {
    id: "folder-1",
    workspaceId: "ws-1",
    collectionId: "col-1",
    parentFolderId: null,
    name: "Auth",
    sortOrder: 0,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
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

describe("useApiCollectionFolders", () => {
  it("loads folders and wires folder/request mutations to commands", async () => {
    listMock.mockResolvedValue([folder()]);
    createMock.mockResolvedValue(folder({ id: "folder-2", name: "Tokens" }));
    renameMock.mockResolvedValue(folder({ name: "Renamed" }));
    moveRequestMock.mockResolvedValue({ id: "req-1" } as Awaited<
      ReturnType<typeof moveApiRequest>
    >);

    const { result } = renderHook(() => useApiCollectionFolders("ws-1"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.folders).toHaveLength(1);

    result.current.createFolderMut.mutate({
      collectionId: "col-1",
      name: "Tokens",
      parentFolderId: "folder-1",
    });
    result.current.renameFolderMut.mutate({
      folderId: "folder-1",
      name: "Renamed",
    });
    result.current.moveRequestMut.mutate({
      collectionId: "col-1",
      parentFolderId: "folder-1",
      requestId: "req-1",
    });

    await waitFor(() =>
      expect(createMock).toHaveBeenCalledWith(
        "ws-1",
        "col-1",
        "folder-1",
        "Tokens",
      ),
    );
    await waitFor(() =>
      expect(renameMock).toHaveBeenCalledWith("ws-1", "folder-1", "Renamed"),
    );
    await waitFor(() =>
      expect(moveRequestMock).toHaveBeenCalledWith(
        "ws-1",
        "req-1",
        "col-1",
        "folder-1",
      ),
    );
  });
});
