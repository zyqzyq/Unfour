// @vitest-environment jsdom
import type { ApiCollection, ApiCollectionFolder } from "@unfour/command-client";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Dialog, DialogContent, DialogDescription, DialogTitle, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, I18nProvider, Popover, PopoverContent, PopoverTrigger } from "@unfour/ui";
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
const sendTab = vi.fn();
const closeTab = vi.fn();
const closeTabs = vi.fn();
let requestTabs: ApiRequestTab[];
const createCollectionMutateAsync = vi.fn();
const createFolderMutateAsync = vi.fn();

vi.mock("./hooks/useApiRequestTabs", () => ({
  useApiRequestTabs: () => {
    const tab = requestTabs[0];
    return {
      activeTab: tab,
      cancelTab: vi.fn(),
      closeTab,
      closeTabs,
      collectionStatus: "",
      newRequest: vi.fn(),
      openHistory: vi.fn(),
      openSaved: vi.fn(),
      saveTab,
      savedRequests: [],
      selectTab: vi.fn(),
      sendTab,
      setCollectionStatus: vi.fn(),
      setRequestTab: vi.fn(),
      setResponseTab: vi.fn(),
      state: { activeTabId: tab.id, splitDirection: "vertical", tabs: requestTabs, workspaceId: "ws-1" },
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

function renderPage(locale: "en" | "zh-CN" = "en", active = true) {
  return render(
    <I18nProvider initialLocale={locale}>
      <ApiClientPage active={active} openIntent={null} workspaceId="ws-1" />
    </I18nProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

beforeEach(() => {
  requestTabs = [unsavedTab()];
  saveTab.mockResolvedValue("req-1");
});

describe("API shortcut ownership", () => {
  it("does not send or save from the cURL preview dialog", () => {
    renderPage();
    fireEvent.click(screen.getByRole("button", { name: "Import cURL" }));
    fireEvent.change(screen.getByLabelText("cURL command"), { target: { value: "curl https://example.test" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    fireEvent.click(screen.getByRole("button", { name: "Create request" }));
    expect(sendTab).not.toHaveBeenCalled();
    expect(saveTab).not.toHaveBeenCalled();
  });
  it.each(["ctrlKey", "metaKey"])("keeps normal send/save and blocks send in the save dialog (%s)", (modifier) => {
    renderPage();
    fireEvent.keyDown(window, { key: "Enter", [modifier]: true });
    expect(sendTab).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(window, { key: "s", [modifier]: true });
    const dialog = screen.getByRole("dialog");
    fireEvent.keyDown(dialog, { key: "Enter", [modifier]: true });
    expect(sendTab).toHaveBeenCalledTimes(1);
  });

  it.each([
    { isComposing: true }, { keyCode: 229 }, { repeat: true },
    { shiftKey: true }, { altKey: true },
  ])("ignores composition, repeats and unrelated modifier combinations: %j", (extra) => {
    renderPage();
    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true, ...extra });
    expect(sendTab).not.toHaveBeenCalled();
  });

  it("lets local editors consume shortcuts", () => {
    renderPage();
    const target = screen.getByRole("button", { name: "Open save" });
    target.addEventListener("keydown", (event) => event.preventDefault());
    fireEvent.keyDown(target, { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(target, { key: "s", metaKey: true });
    expect(sendTab).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("blocks send while a context menu is open even if the event targets the window", () => {
    renderPage();
    fireEvent.contextMenu(screen.getByRole("tab"));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    expect(screen.queryByText("Ctrl+W")).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(sendTab).not.toHaveBeenCalled();
  });

  it("does not handle shortcuts while inactive", () => {
    renderPage("en", false);
    fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(sendTab).not.toHaveBeenCalled();
  });

  it.each(["dialog", "dropdown", "popover"])("yields to an external %s layer", (layer) => {
    renderPage();
    render(layer === "dialog" ? (
      <Dialog open><DialogContent><DialogTitle>External dialog</DialogTitle><DialogDescription>Test</DialogDescription><input aria-label="Layer input" /></DialogContent></Dialog>
    ) : layer === "dropdown" ? (
      <DropdownMenu open><DropdownMenuTrigger>Menu</DropdownMenuTrigger><DropdownMenuContent><DropdownMenuItem>Action</DropdownMenuItem></DropdownMenuContent></DropdownMenu>
    ) : (
      <Popover open><PopoverTrigger>Popover</PopoverTrigger><PopoverContent><input aria-label="Layer input" /></PopoverContent></Popover>
    ));
    fireEvent.keyDown(document.activeElement!, { key: "Enter", metaKey: true });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    expect(sendTab).not.toHaveBeenCalled();
    expect(screen.queryByDisplayValue("Health check")).not.toBeInTheDocument();
  });

  it("sends from an ordinary input and saves an existing request", () => {
    requestTabs[0] = { ...requestTabs[0], savedRequestId: "req-1" };
    renderPage();
    render(<input aria-label="Editor input" />);
    const input = screen.getByRole("textbox", { name: "Editor input" });
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    expect(sendTab).toHaveBeenCalledExactlyOnceWith(requestTabs[0]);
    fireEvent.keyDown(input, { key: "s", metaKey: true });
    expect(saveTab).toHaveBeenCalledExactlyOnceWith(requestTabs[0]);
  });
});

describe("API unsaved close queue", () => {
  it("returns focus to the active request after cancelling a close opened from its menu", async () => {
    renderPage();
    const tab = screen.getByRole("tab");
    tab.focus();
    fireEvent.contextMenu(tab);
    const item = screen.getByRole("menuitem", { name: "Close" });
    item.focus();
    fireEvent.click(item);
    fireEvent.keyDown(await screen.findByRole("dialog"), { key: "Escape" });
    await waitFor(() => expect(tab).toHaveFocus());
    expect(closeTab).not.toHaveBeenCalled();
  });

  it.each([
    ["en", "Close Health check", "Save changes?", "Cancel", "Don't Save"],
    ["zh-CN", "关闭 Health check", "保存更改？", "取消", "不保存"],
  ] as const)("localizes the close dialog and cancels on Escape (%s)", async (locale, close, title, cancel) => {
    renderPage(locale);
    const trigger = screen.getByRole("button", { name: close });
    trigger.focus();
    fireEvent.click(trigger);
    const dialog = screen.getByRole("dialog", { name: title });
    expect(screen.getByRole("button", { name: cancel })).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Enter", ctrlKey: true });
    expect(sendTab).not.toHaveBeenCalled();
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(closeTab).not.toHaveBeenCalled();
    expect(trigger).toHaveFocus();
  });

  it.each(["Close All", "Close Tabs to the Right"])("confirms every dirty target and cancels the rest of %s", async (action) => {
    requestTabs = [unsavedTab(), { ...unsavedTab(), id: "new:2" }, { ...unsavedTab(), id: "new:3" }];
    renderPage();
    fireEvent.contextMenu(screen.getAllByRole("tab")[0]);
    fireEvent.click(screen.getByRole("menuitem", { name: action }));
    await screen.findByRole("dialog");
    expect(closeTab).not.toHaveBeenCalled();
    expect(closeTabs).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Don't Save" }));
    expect(closeTab).toHaveBeenCalledExactlyOnceWith(action === "Close All" ? "new:1" : "new:2");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(closeTab).toHaveBeenCalledTimes(1);
    expect(closeTabs).not.toHaveBeenCalled();
  });

  it("waits for a successful save before closing and continues the pending batch", async () => {
    requestTabs = [{ ...unsavedTab(), savedRequestId: "req-1" }, { ...unsavedTab(), id: "new:2" }];
    const pending = deferred<string>();
    saveTab.mockReturnValue(pending.promise);
    renderPage();
    fireEvent.contextMenu(screen.getAllByRole("tab")[0]);
    fireEvent.click(screen.getByRole("menuitem", { name: "Close All" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(saveTab).toHaveBeenCalledExactlyOnceWith(requestTabs[0]);
    expect(closeTab).not.toHaveBeenCalled();
    pending.resolve("req-1");
    await waitFor(() => expect(closeTab).toHaveBeenCalledExactlyOnceWith("new:1"));
    expect(screen.getByRole("dialog", { name: "Save changes?" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(closeTab).toHaveBeenCalledTimes(1);
  });

  it("cancelling a new request's save dialog abandons the close queue", async () => {
    requestTabs = [unsavedTab(), { ...unsavedTab(), id: "new:2" }];
    renderPage();
    fireEvent.contextMenu(screen.getAllByRole("tab")[0]);
    fireEvent.click(screen.getByRole("menuitem", { name: "Close All" }));
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    expect(screen.getByDisplayValue("Health check")).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(closeTab).not.toHaveBeenCalled();
    expect(closeTabs).not.toHaveBeenCalled();
    expect(saveTab).not.toHaveBeenCalled();
  });
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
