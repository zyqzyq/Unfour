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
  exportApiCollection: vi.fn(),
  importApiCollection: vi.fn(),
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
  exportApiCollection,
  importApiCollection,
  listApiCollections,
  listApiCollectionFolders,
  listApiHistory,
  listSavedApiRequests,
  moveApiCollectionFolder,
  moveApiRequest,
  reorderApiCollectionFolders,
  reorderApiRequests,
  updateApiRequest,
} from "@unfour/command-client";

const listCollectionsMock = vi.mocked(listApiCollections);
const listFoldersMock = vi.mocked(listApiCollectionFolders);
const listSavedMock = vi.mocked(listSavedApiRequests);
const listHistoryMock = vi.mocked(listApiHistory);
const duplicateMock = vi.mocked(duplicateApiRequest);
const exportMock = vi.mocked(exportApiCollection);
const importMock = vi.mocked(importApiCollection);
const moveFolderMock = vi.mocked(moveApiCollectionFolder);
const moveMock = vi.mocked(moveApiRequest);
const reorderFoldersMock = vi.mocked(reorderApiCollectionFolders);
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
  exportMock.mockResolvedValue({ saved: true });
  importMock.mockResolvedValue({
    imported: true,
    collection: collection({ id: "col-imported", name: "Imported Users" }),
    folderCount: 2,
    requestCount: 3,
  });
  moveFolderMock.mockResolvedValue(folder({ parentFolderId: "folder-2" }));
  moveMock.mockResolvedValue(savedRequest({ parentFolderId: "folder-1" }));
  reorderFoldersMock.mockResolvedValue([folder()]);
  reorderRequestsMock.mockResolvedValue([savedRequest()]);
  updateMock.mockResolvedValue(savedRequest({ name: "List Users" }));
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ApiCollectionTree", () => {
  it("imports an Unfour collection export from the collections toolbar", async () => {
    renderTree();

    fireEvent.click(await screen.findByRole("button", { name: "Import collection" }));

    await waitFor(() => expect(importMock).toHaveBeenCalledWith("ws-1"));
  });

  it("exports a collection as OpenAPI YAML from the collection context menu", async () => {
    renderTree();

    const collectionRow = (await screen.findByText("Users")).closest("[role='treeitem']");
    expect(collectionRow).not.toBeNull();
    fireEvent.contextMenu(collectionRow as HTMLElement, { clientX: 24, clientY: 24 });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Export" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "OpenAPI 3.1 YAML" }),
    );

    await waitFor(() =>
      expect(exportMock).toHaveBeenCalledWith("ws-1", "col-1", "yaml"),
    );
  });

  it("shows request row actions without an export action", async () => {
    renderTree();

    const actions = await screen.findByRole("button", {
      name: "Request actions for Get Users",
    });
    fireEvent.pointerDown(actions);

    await screen.findByRole("menuitem", { name: "Open in tab" });
    expect(screen.queryByRole("menuitem", { name: "Export" })).not.toBeInTheDocument();
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
    expect(contextLabels).not.toContain("Export");
    expect(contextLabels).not.toContain("Move to");
  });

  it("does not show export in a folder context menu", async () => {
    listFoldersMock.mockResolvedValue([folder()]);
    renderTree();

    const folderRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    expect(folderRow).not.toBeNull();
    fireEvent.contextMenu(folderRow as HTMLElement, { clientX: 24, clientY: 24 });

    await screen.findByRole("menuitem", { name: "Add folder" });
    expect(screen.queryByRole("menuitem", { name: "Export" })).not.toBeInTheDocument();
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
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    HTMLElement.prototype.getBoundingClientRect = () =>
      ({
        bottom: 24,
        height: 24,
        left: 0,
        right: 180,
        top: 0,
        width: 180,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect;

    try {
      const transfer = dataTransfer();
      fireEvent.dragStart(sourceRow as HTMLElement, { dataTransfer: transfer });
      fireEvent.dragOver(targetRow as HTMLElement, {
        clientY: 1,
        dataTransfer: transfer,
      });
      expect(targetRow).toHaveAttribute("data-drop-position", "before");
      fireEvent.drop(targetRow as HTMLElement, {
        clientY: 1,
        dataTransfer: transfer,
      });
    } finally {
      HTMLElement.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    }

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

  it("reorders sibling folders by dragging before another folder", async () => {
    listFoldersMock.mockResolvedValue([
      folder({ id: "folder-1", name: "Auth", sortOrder: 0 }),
      folder({ id: "folder-2", name: "Billing", sortOrder: 1 }),
    ]);
    listSavedMock.mockResolvedValue([]);
    renderTree();

    const sourceLabel = (await screen.findByText("Billing")).closest("button");
    const targetRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    expect(sourceLabel).not.toBeNull();
    expect(targetRow).not.toBeNull();
    expect(targetRow).toHaveAttribute("data-tree-id", "folder:folder-1");

    const originalElementFromPoint = document.elementFromPoint;
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: vi.fn(() => targetRow as Element),
    });
    try {
      fireEvent.pointerDown(sourceLabel as HTMLElement, {
        button: 0,
        clientX: 12,
        clientY: 12,
        pointerId: 1,
      });
      fireEvent.pointerMove(sourceLabel as HTMLElement, {
        clientX: 28,
        clientY: 1,
        pointerId: 1,
      });
      expect(targetRow).toHaveAttribute("data-drop-position", "before");
      fireEvent.pointerUp(sourceLabel as HTMLElement, {
        clientX: 28,
        clientY: 1,
        pointerId: 1,
      });
    } finally {
      Object.defineProperty(document, "elementFromPoint", {
        configurable: true,
        value: originalElementFromPoint,
      });
    }

    await waitFor(() =>
      expect(reorderFoldersMock).toHaveBeenCalledWith(
        "ws-1",
        "col-1",
        null,
        ["folder-2", "folder-1"],
      ),
    );
    expect(moveFolderMock).not.toHaveBeenCalled();
  });

  it("moves a folder into another folder by dragging onto it", async () => {
    listFoldersMock.mockResolvedValue([
      folder({ id: "folder-1", name: "Auth", sortOrder: 0 }),
      folder({ id: "folder-2", name: "Billing", sortOrder: 1 }),
    ]);
    listSavedMock.mockResolvedValue([]);
    renderTree();

    const sourceRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    const targetRow = (await screen.findByText("Billing")).closest("[role='treeitem']");
    expect(sourceRow).not.toBeNull();
    expect(targetRow).not.toBeNull();

    const transfer = dataTransfer();
    fireEvent.dragStart(sourceRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(targetRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.drop(targetRow as HTMLElement, { dataTransfer: transfer });

    await waitFor(() =>
      expect(moveFolderMock).toHaveBeenCalledWith("ws-1", "folder-1", "folder-2"),
    );
    expect(reorderFoldersMock).not.toHaveBeenCalled();
  });

  it("does not move a folder into its own child folder", async () => {
    listFoldersMock.mockResolvedValue([
      folder({ id: "folder-1", name: "Auth", parentFolderId: null, sortOrder: 0 }),
      folder({
        id: "folder-2",
        name: "Tokens",
        parentFolderId: "folder-1",
        sortOrder: 0,
      }),
    ]);
    listSavedMock.mockResolvedValue([]);
    renderTree();

    const sourceRow = (await screen.findByText("Auth")).closest("[role='treeitem']");
    const targetRow = (await screen.findByText("Tokens")).closest("[role='treeitem']");
    expect(sourceRow).not.toBeNull();
    expect(targetRow).not.toBeNull();

    const transfer = dataTransfer();
    fireEvent.dragStart(sourceRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(targetRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.drop(targetRow as HTMLElement, { dataTransfer: transfer });

    expect(moveFolderMock).not.toHaveBeenCalled();
    expect(reorderFoldersMock).not.toHaveBeenCalled();
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
