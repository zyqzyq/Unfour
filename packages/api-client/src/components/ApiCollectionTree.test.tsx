// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiCollection, ApiSavedRequest } from "@unfour/command-client";
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
  updateApiRequest,
} from "@unfour/command-client";

const listCollectionsMock = vi.mocked(listApiCollections);
const listFoldersMock = vi.mocked(listApiCollectionFolders);
const listSavedMock = vi.mocked(listSavedApiRequests);
const listHistoryMock = vi.mocked(listApiHistory);
const duplicateMock = vi.mocked(duplicateApiRequest);
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
});
