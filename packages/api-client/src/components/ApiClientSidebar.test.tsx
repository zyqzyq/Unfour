// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClientSidebar } from "./ApiClientSidebar";

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
  listApiCollections,
  listApiCollectionFolders,
  listApiHistory,
  listSavedApiRequests,
} from "@unfour/command-client";

const listCollectionsMock = vi.mocked(listApiCollections);
const listFoldersMock = vi.mocked(listApiCollectionFolders);
const listSavedMock = vi.mocked(listSavedApiRequests);
const listHistoryMock = vi.mocked(listApiHistory);
function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <I18nProvider initialLocale="en">
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </I18nProvider>
    );
  }
  return { client, Wrapper };
}

function renderSidebar(overrides: Partial<Parameters<typeof ApiClientSidebar>[0]> = {}) {
  const props = {
    onNewRequest: vi.fn(),
    onOpenIntent: vi.fn(),
    selectedId: null,
    workspaceId: "ws-1",
    ...overrides,
  };
  const { client, Wrapper } = createWrapper();
  render(<ApiClientSidebar {...props} />, { wrapper: Wrapper });
  return { ...props, queryClient: client };
}

beforeEach(() => {
  vi.clearAllMocks();
  listCollectionsMock.mockResolvedValue([]);
  listFoldersMock.mockResolvedValue([]);
  listSavedMock.mockResolvedValue([]);
  listHistoryMock.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ApiClientSidebar", () => {
  it("keeps API navigation limited to collections and history", () => {
    renderSidebar();

    expect(screen.getByRole("button", { name: "Collections" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "History" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Environments" })).toBeNull();
  });

  it("switches to API history without opening workspace variable management", async () => {
    renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText("Send a request to build history.")).toBeInTheDocument();
  });

  it("shows a loading state while history is fetching", () => {
    listHistoryMock.mockReturnValue(new Promise(() => undefined));
    renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "History" }));
    expect(screen.getByText("Loading...")).toBeInTheDocument();
    expect(screen.queryByText("Send a request to build history.")).not.toBeInTheDocument();
  });

  it("shows a history error with retry when the first load fails", async () => {
    listHistoryMock.mockRejectedValue(new Error("offline"));
    renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "History" }));

    expect(await screen.findByText("Failed to load request history.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.queryByText("Send a request to build history.")).not.toBeInTheDocument();
  });

  it("retries a failed history load", async () => {
    listHistoryMock
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce([]);
    renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "History" }));
    fireEvent.click(await screen.findByRole("button", { name: "Retry" }));

    expect(await screen.findByText("Send a request to build history.")).toBeInTheDocument();
    expect(listHistoryMock).toHaveBeenCalledTimes(2);
  });

  it("keeps previously loaded history when a later refresh fails", async () => {
    listHistoryMock.mockResolvedValueOnce([
      {
        createdAt: "2026-06-15T00:00:00Z",
        deletedAt: null,
        durationMs: 12,
        id: "h1",
        method: "GET",
        name: "Health check",
        remoteId: null,
        revision: 1,
        status: 200,
        syncStatus: "local",
        updatedAt: "2026-06-15T00:00:00Z",
        url: "https://example.test",
        workspaceId: "ws-1",
      },
    ]);
    const { queryClient } = renderSidebar();
    fireEvent.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText("Health check")).toBeInTheDocument();

    listHistoryMock.mockRejectedValueOnce(new Error("offline"));
    await queryClient.refetchQueries({ queryKey: ["api-history", "ws-1"] });

    expect(await screen.findByText("Failed to load request history.")).toBeInTheDocument();
    expect(screen.getByText("Health check")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
