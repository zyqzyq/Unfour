// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiCollection, ApiCollectionFolder, ApiSavedRequest } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { ApiCollectionTree } from "./ApiCollectionTree";

vi.mock("@unfour/command-client", () => ({
  createApiCollection: vi.fn(),
  createApiCollectionFolder: vi.fn(),
  deleteApiCollection: vi.fn(),
  deleteApiCollectionFolder: vi.fn(),
  deleteApiRequest: vi.fn(),
  duplicateApiRequest: vi.fn(),
  listApiCollections: vi.fn(),
  listApiCollectionFolders: vi.fn(),
  listApiHistory: vi.fn(),
  listSavedApiRequests: vi.fn(),
  moveApiCollectionFolder: vi.fn(),
  moveApiRequest: vi.fn(),
  renameApiCollection: vi.fn(),
  renameApiCollectionFolder: vi.fn(),
  reorderApiCollectionFolders: vi.fn(),
  reorderApiRequests: vi.fn(),
  updateApiRequest: vi.fn(),
}));

import {
  duplicateApiRequest,
  listApiCollections,
  listApiCollectionFolders,
  listApiHistory,
  listSavedApiRequests,
  moveApiRequest,
  reorderApiRequests,
  updateApiRequest,
} from "@unfour/command-client";

const listCollectionsMock = vi.mocked(listApiCollections);
const listFoldersMock = vi.mocked(listApiCollectionFolders);
const listSavedMock = vi.mocked(listSavedApiRequests);
const listHistoryMock = vi.mocked(listApiHistory);
const duplicateMock = vi.mocked(duplicateApiRequest);
const moveMock = vi.mocked(moveApiRequest);
const reorderRequestsMock = vi.mocked(reorderApiRequests);
const updateMock = vi.mocked(updateApiRequest);

function collection(overrides: Partial<ApiCollection> = {}): ApiCollection {
  return {
    id: "col-1",
    workspaceId: "ws-1",
    name: "Users",
    description: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function savedRequest(overrides: Partial<ApiSavedRequest> = {}): ApiSavedRequest {
  return {
    id: "req-1",
    workspaceId: "ws-1",
    name: "Get Users",
    collectionId: "col-1",
    parentFolderId: null,
    sortOrder: 0,
    method: "GET",
    url: "https://api.example.com/users",
    headersJson: "[]",
    queryJson: "[]",
    body: null,
    bodyKind: "none",
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
    ...overrides,
  };
}

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

function dataTransfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    clearData: vi.fn((format?: string) => {
      if (format) {
        data.delete(format);
      } else {
        data.clear();
      }
    }),
    dropEffect: "move",
    effectAllowed: "all",
    files: [] as unknown as FileList,
    getData: vi.fn((format: string) => data.get(format) ?? ""),
    items: [] as unknown as DataTransferItemList,
    setData: vi.fn((format: string, value: string) => data.set(format, value)),
    types: [],
    setDragImage: vi.fn(),
  };
}

function menuItemLabels() {
  return screen
    .getAllByRole("menuitem")
    .map((item) => item.textContent?.trim() ?? "");
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <I18nProvider initialLocale="en">
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </I18nProvider>
    );
  };
}

function renderTree() {
  return render(
    <ApiCollectionTree
      active
      collapsed={false}
      onOpenClient={vi.fn()}
      onOpenIntent={vi.fn()}
      selectedId={null}
      workspaceId="ws-1"
    />,
    { wrapper: createWrapper() },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  listCollectionsMock.mockResolvedValue([collection()]);
  listFoldersMock.mockResolvedValue([]);
  listSavedMock.mockResolvedValue([savedRequest()]);
  listHistoryMock.mockResolvedValue([]);
  duplicateMock.mockResolvedValue(savedRequest({ id: "req-2", name: "Get Users Copy" }));
  moveMock.mockResolvedValue(savedRequest({ parentFolderId: "folder-1" }));
  reorderRequestsMock.mockResolvedValue([savedRequest()]);
  updateMock.mockResolvedValue(savedRequest({ name: "List Users" }));
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ApiCollectionTree", () => {
  it("shows a row action menu for saved requests", async () => {
    renderTree();

    expect(
      await screen.findByRole("button", { name: "Request actions for Get Users" }),
    ).toBeInTheDocument();
  });

  it("renames a saved request from the row action menu", async () => {
    renderTree();

    fireEvent.pointerDown(
      await screen.findByRole("button", { name: "Request actions for Get Users" }),
    );
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename request" }));
    fireEvent.change(screen.getByDisplayValue("Get Users"), {
      target: { value: "List Users" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(updateMock).toHaveBeenCalledWith(
        "ws-1",
        "req-1",
        expect.objectContaining({
          name: "List Users",
          method: "GET",
          url: "https://api.example.com/users",
        }),
      ),
    );
  });

  it("duplicates a saved request from the row action menu", async () => {
    renderTree();

    fireEvent.pointerDown(
      await screen.findByRole("button", { name: "Request actions for Get Users" }),
    );
    fireEvent.click(await screen.findByRole("menuitem", { name: "Duplicate" }));

    await waitFor(() => expect(duplicateMock).toHaveBeenCalledWith("ws-1", "req-1"));
  });

  it("uses the same request actions in row and context menus without move targets", async () => {
    listFoldersMock.mockResolvedValue([folder()]);
    const first = renderTree();

    fireEvent.pointerDown(
      await screen.findByRole("button", { name: "Request actions for Get Users" }),
    );
    await screen.findByRole("menuitem", { name: "Open in tab" });
    const rowLabels = menuItemLabels();
    expect(rowLabels).not.toContain("Move to");

    first.unmount();
    cleanup();

    renderTree();
    const requestRow = (await screen.findByText("Get Users")).closest("[role='treeitem']");
    expect(requestRow).not.toBeNull();
    fireEvent.contextMenu(requestRow as HTMLElement, { clientX: 24, clientY: 24 });

    await screen.findByRole("menuitem", { name: "Open in tab" });
    const contextLabels = menuItemLabels();
    expect(contextLabels).toEqual(rowLabels);
    expect(contextLabels).not.toContain("Move to");
  });

  it("moves a saved request by dragging it onto a folder", async () => {
    listFoldersMock.mockResolvedValue([folder()]);
    renderTree();

    const requestRow = (await screen.findByText("Get Users")).closest("[role='treeitem']");
    const folderRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    expect(requestRow).not.toBeNull();
    expect(folderRow).not.toBeNull();

    const transfer = dataTransfer();
    fireEvent.dragStart(requestRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(folderRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.drop(folderRow as HTMLElement, { dataTransfer: transfer });

    await waitFor(() =>
      expect(moveMock).toHaveBeenCalledWith("ws-1", "req-1", "col-1", "folder-1"),
    );
  });

  it("reorders sibling saved requests by dragging before another request", async () => {
    listSavedMock.mockResolvedValue([
      savedRequest({ id: "req-1", name: "Get Users", sortOrder: 0 }),
      savedRequest({ id: "req-2", name: "List Teams", sortOrder: 1 }),
    ]);
    renderTree();

    const sourceRow = (await screen.findByText("List Teams")).closest("[role='treeitem']");
    const targetRow = (await screen.findByText("Get Users")).closest("[role='treeitem']");
    expect(sourceRow).not.toBeNull();
    expect(targetRow).not.toBeNull();
    vi.spyOn(targetRow as HTMLElement, "getBoundingClientRect").mockReturnValue({
      bottom: 24,
      height: 24,
      left: 0,
      right: 180,
      top: 0,
      width: 180,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });

    const transfer = dataTransfer();
    fireEvent.dragStart(sourceRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(targetRow as HTMLElement, {
      clientY: 1,
      dataTransfer: transfer,
    });
    fireEvent.drop(targetRow as HTMLElement, {
      clientY: 1,
      dataTransfer: transfer,
    });

    await waitFor(() =>
      expect(reorderRequestsMock).toHaveBeenCalledWith(
        "ws-1",
        "col-1",
        null,
        ["req-2", "req-1"],
      ),
    );
    expect(moveMock).not.toHaveBeenCalled();
  });

  it("moves a saved request by pointer dragging the request label onto a folder", async () => {
    listFoldersMock.mockResolvedValue([folder()]);
    renderTree();

    const requestLabel = (await screen.findByText("Get Users")).closest("button");
    const folderRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    expect(requestLabel).not.toBeNull();
    expect(folderRow).not.toBeNull();

    const originalElementFromPoint = document.elementFromPoint;
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: vi.fn(() => folderRow as Element),
    });

    try {
      fireEvent.pointerDown(requestLabel as HTMLElement, {
        button: 0,
        clientX: 12,
        clientY: 12,
        pointerId: 1,
      });
      fireEvent.pointerMove(requestLabel as HTMLElement, {
        clientX: 28,
        clientY: 28,
        pointerId: 1,
      });
      fireEvent.pointerUp(requestLabel as HTMLElement, {
        clientX: 28,
        clientY: 28,
        pointerId: 1,
      });

      await waitFor(() =>
        expect(moveMock).toHaveBeenCalledWith("ws-1", "req-1", "col-1", "folder-1"),
      );
    } finally {
      Object.defineProperty(document, "elementFromPoint", {
        configurable: true,
        value: originalElementFromPoint,
      });
    }
  });
});
