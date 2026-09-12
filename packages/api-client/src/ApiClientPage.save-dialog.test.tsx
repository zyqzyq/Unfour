// @vitest-environment jsdom
import type { ApiCollection, ApiCollectionFolder } from "@unfour/command-client";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createNewRequestTab,
  emptyApiTabsState,
  type ApiRequestTab,
} from "./model/request-tabs";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  promise.catch(() => undefined);
  return { promise, reject, resolve };
}

function unsavedTab(): ApiRequestTab {
  const opened = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
  const tab = opened.tabs[0];
  return {
    ...tab,
    draft: {
      ...tab.draft,
      collectionId: "col-1",
      name: "Health check",
    },
  };
}

function collection(): ApiCollection {
  return {
    id: "col-1",
    workspaceId: "ws-1",
    name: "Default",
    description: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

const saveTab = vi.fn();
const createCollectionMutateAsync = vi.fn();
const createFolderMutateAsync = vi.fn();

vi.mock("./hooks/useApiRequestTabs", () => ({
  useApiRequestTabs: () => {
    const tab = unsavedTab();
    return {
      activeTab: tab,
      cancelTab: vi.fn(),
      closeTab: vi.fn(),
      closeTabs: vi.fn(),
      collectionStatus: "",
      newRequest: vi.fn(),
      openHistory: vi.fn(),
      openSaved: vi.fn(),
      saveTab,
      savedRequests: [],
      selectTab: vi.fn(),
      sendTab: vi.fn(),
      setCollectionStatus: vi.fn(),
      setRequestTab: vi.fn(),
      setResponseTab: vi.fn(),
      state: { activeTabId: tab.id, splitDirection: "vertical", tabs: [tab], workspaceId: "ws-1" },
      updateDraft: vi.fn(),
    };
  },
}));

vi.mock("./hooks/useApiCollections", () => ({
  useApiCollections: () => ({
    collections: [collection()],
    createMut: { mutateAsync: createCollectionMutateAsync },
  }),
}));

vi.mock("./hooks/useApiCollectionFolders", () => ({
  useApiCollectionFolders: () => ({
    createFolderMut: { mutateAsync: createFolderMutateAsync },
    folders: [],
  }),
}));

vi.mock("./components/ApiRequestTabs", () => ({
  ApiRequestTabs: () => null,
}));

vi.mock("./components/ApiClientSidebar", () => ({
  ApiClientSidebar: () => null,
}));

vi.mock("./components/ApiRequestWorkspace", () => ({
  ApiRequestWorkspace: ({
    activeTab,
    onSave,
  }: {
    activeTab: ApiRequestTab;
    onSave: (tab: ApiRequestTab) => void;
  }) => (
    <button onClick={() => onSave(activeTab)} type="button">
      Open save
    </button>
  ),
}));

import { ApiClientPage } from "./ApiClientPage";

function renderPage() {
  return render(
    <I18nProvider initialLocale="en">
      <ApiClientPage openIntent={null} workspaceId="ws-1" />
    </I18nProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

beforeEach(() => {
  saveTab.mockResolvedValue("req-1");
});

describe("ApiClientPage save dialog pending", () => {
  it("ignores repeat submits while creating a collection and restores after failure", async () => {
    const pending = deferred<ApiCollection>();
    createCollectionMutateAsync.mockReturnValue(pending.promise);
    renderPage();

    fireEvent.click(screen.getByRole("button", { name: "Open save" }));
    fireEvent.click(screen.getByRole("button", { name: "+ New collection" }));
    fireEvent.change(screen.getByPlaceholderText("Collection name"), {
      target: { value: "Auth APIs" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("button", { name: "Saving" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Saving" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(createCollectionMutateAsync).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    pending.reject(new Error("collection exists"));
    await waitFor(() => {
      expect(screen.getByRole("dialog")).toHaveTextContent("collection exists");
      expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    });
    expect(screen.getByDisplayValue("Health check")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Auth APIs")).toBeInTheDocument();
    expect(saveTab).not.toHaveBeenCalled();

    const retry = deferred<ApiCollection>();
    createCollectionMutateAsync.mockReturnValue(retry.promise);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("button", { name: "Saving" })).toBeDisabled();
    expect(createCollectionMutateAsync).toHaveBeenCalledTimes(2);
  });

  it("ignores repeat submits while creating a folder", async () => {
    const pending = deferred<ApiCollectionFolder>();
    createFolderMutateAsync.mockReturnValue(pending.promise);
    renderPage();

    fireEvent.click(screen.getByRole("button", { name: "Open save" }));
    fireEvent.change(screen.getByPlaceholderText("Subfolder name"), {
      target: { value: "Auth" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("button", { name: "Saving" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Saving" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(createFolderMutateAsync).toHaveBeenCalledTimes(1);
    expect(createCollectionMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    pending.reject(new Error("folder exists"));
    await waitFor(() => {
      expect(screen.getByRole("dialog")).toHaveTextContent("folder exists");
      expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
    });
    expect(screen.getByDisplayValue("Health check")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Auth")).toBeInTheDocument();
    expect(saveTab).not.toHaveBeenCalled();
  });
});
