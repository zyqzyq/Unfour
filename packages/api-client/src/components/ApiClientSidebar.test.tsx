// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <I18nProvider initialLocale="en">
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </I18nProvider>
    );
  };
}

function renderSidebar(overrides: Partial<Parameters<typeof ApiClientSidebar>[0]> = {}) {
  const props = {
    onNewRequest: vi.fn(),
    onOpenIntent: vi.fn(),
    selectedId: null,
    workspaceId: "ws-1",
    ...overrides,
  };
  render(<ApiClientSidebar {...props} />, { wrapper: createWrapper() });
  return props;
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
});
