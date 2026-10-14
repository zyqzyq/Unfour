// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import * as commands from "@unfour/command-client";
import { DatabasePage } from "./DatabasePage";
import { resetDatabaseTabStore, useDatabaseTabStore } from "./model/database-tab-state";
import { useDatabaseConnectionStore } from "./model/database-connection-state";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listDatabaseConnections: vi.fn(), listDatabaseQueryHistory: vi.fn(), listSavedSql: vi.fn(),
  saveDatabaseConnection: vi.fn(),
}));
vi.mock("./components/DatabaseWorkspace", () => ({ DatabaseWorkspace: () => <div>Existing database workspace</div> }));
beforeEach(() => {
  vi.clearAllMocks();
  resetDatabaseTabStore();
  useWorkspaceStore.setState({ selectedDatabaseConnectionId: null });
  useDatabaseConnectionStore.setState({ byWorkspace: {} });
  vi.mocked(commands.listDatabaseConnections).mockResolvedValue([]);
  vi.mocked(commands.listDatabaseQueryHistory).mockResolvedValue([]);
  vi.mocked(commands.listSavedSql).mockResolvedValue([]);
});
afterEach(cleanup);
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><I18nProvider initialLocale="en"><DatabasePage workspaceId="ws" /></I18nProvider></QueryClientProvider>);
}

it("offers the existing connection dialog without creating or executing anything", async () => {
  mount();
  fireEvent.click(await screen.findByRole("button", { name: "Add connection" }));
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(commands.saveDatabaseConnection).not.toHaveBeenCalled();
  expect(screen.queryByText("Existing database workspace")).not.toBeInTheDocument();
});

it("distinguishes pending and failed loading from a genuinely empty workspace and retries", async () => {
  let reject!: (error: Error) => void;
  vi.mocked(commands.listDatabaseConnections).mockImplementationOnce(() => new Promise((_resolve, fail) => { reject = fail; }));
  mount();
  expect(screen.getByText("Loading database connections…")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Add connection" })).not.toBeInTheDocument();
  await act(async () => reject(new Error("Offline")));
  expect(await screen.findByRole("alert")).toHaveTextContent("could not be loaded");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(await screen.findByRole("button", { name: "Add connection" })).toBeVisible();
  expect(commands.listDatabaseConnections).toHaveBeenCalledTimes(2);
});

it("keeps an existing SQL draft reachable when the connection list is empty", async () => {
  useDatabaseTabStore.getState().openQueryTab("ws", { sql: "select 42" });
  mount();
  await waitFor(() => expect(commands.listDatabaseConnections).toHaveBeenCalled());
  expect(screen.getByText("Existing database workspace")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Add connection" })).not.toBeInTheDocument();
  expect(useDatabaseTabStore.getState().byWorkspace.ws.tabs.some((tab) => tab.kind === "query" && tab.sql === "select 42")).toBe(true);
});

it("does not hide a draft whose saved SQL has been cleared", async () => {
  const id = useDatabaseTabStore.getState().openQueryTab("ws", { sql: "select 42" });
  useDatabaseTabStore.getState().updateQueryTab("ws", id, { sql: "" });
  mount();
  await waitFor(() => expect(commands.listDatabaseConnections).toHaveBeenCalled());
  expect(screen.getByText("Existing database workspace")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Add connection" })).not.toBeInTheDocument();
});
