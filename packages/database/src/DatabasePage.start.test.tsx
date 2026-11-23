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
import { connection, deferred } from "./hooks/database-controller.test-fixtures";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listDatabaseConnections: vi.fn(), listDatabaseQueryHistory: vi.fn(), listSavedSql: vi.fn(),
  listRedisConnections: vi.fn(), listSshConnections: vi.fn(), listTemporaryRoutes: vi.fn(),
  saveDatabaseConnection: vi.fn(), saveRedisConnection: vi.fn(),
  testDatabaseConnectionInput: vi.fn(), testRedisConnectionInput: vi.fn(),
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
  vi.mocked(commands.listRedisConnections).mockResolvedValue([]);
  vi.mocked(commands.listSshConnections).mockResolvedValue([]);
  vi.mocked(commands.listTemporaryRoutes).mockResolvedValue([]);
});
afterEach(cleanup);
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const page = (workspaceId: string) => <QueryClientProvider client={client}><I18nProvider initialLocale="en"><DatabasePage workspaceId={workspaceId} /></I18nProvider></QueryClientProvider>;
  const view = render(page("ws"));
  return { showWorkspace: (workspaceId: string) => view.rerender(page(workspaceId)) };
}

async function openConnectionEditor() {
  fireEvent.click(await screen.findByRole("button", { name: "Add connection" }));
}

function typeSelect() {
  return screen.getByRole("combobox", { name: "Database type" });
}

function choose(type: string) {
  fireEvent.change(typeSelect(), { target: { value: type } });
}

function enter(label: string, value: string) {
  const field = label === "Password" ? passwordInput() : screen.getByLabelText(label, { exact: true });
  fireEvent.change(field, { target: { value } });
}

function passwordInput() {
  return screen.getByLabelText(/^Password/);
}

function expectNoConnectionCommands() {
  expect(commands.saveDatabaseConnection).not.toHaveBeenCalled();
  expect(commands.saveRedisConnection).not.toHaveBeenCalled();
  expect(commands.testDatabaseConnectionInput).not.toHaveBeenCalled();
  expect(commands.testRedisConnectionInput).not.toHaveBeenCalled();
}

it("offers the existing connection dialog without creating or executing anything", async () => {
  mount();
  await openConnectionEditor();
  expect(screen.getByRole("dialog")).toBeVisible();
  expectNoConnectionCommands();
  expect(screen.queryByText("Existing database workspace")).not.toBeInTheDocument();
});

it("switches SQL → Redis → SQL in one dialog and submits a fresh SQL draft", async () => {
  vi.mocked(commands.saveDatabaseConnection).mockResolvedValue(connection("App data", "mysql"));
  mount();
  await openConnectionEditor();
  const dialog = screen.getByRole("dialog");
  enter("Name", "App data");
  choose("postgres");
  expect(screen.getByLabelText("Port")).toHaveValue(5432);
  enter("Host", "sql.example.test");
  enter("Username", "sql-user");
  enter("Password", "sql-secret");
  enter("Database", "sql-database");
  choose("redis");
  expect(screen.getByRole("dialog")).toBe(dialog);
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("App data");
  expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  expect(screen.getByLabelText("Port")).toHaveValue(6379);
  expect(passwordInput()).toHaveValue("");
  enter("Name", "Renamed app data");
  enter("Password", "redis-secret");
  choose("mysql");
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("Renamed app data");
  expect(screen.getByLabelText("Host")).toHaveValue("");
  expect(screen.getByLabelText("Port")).toHaveValue(3306);
  expect(passwordInput()).toHaveValue("");
  expectNoConnectionCommands();
  fireEvent.click(screen.getByRole("button", { name: "Save", exact: true }));
  await waitFor(() => expect(commands.saveDatabaseConnection).toHaveBeenCalledWith({
    workspaceId: "ws", name: "Renamed app data", driver: "mysql", port: 3306,
    credentialRef: null, sqlitePath: null, host: null, database: null, username: null, sslMode: null,
  }, null));
});

const savedRedis: commands.RedisConnection = {
  id: "cache", workspaceId: "ws", name: "Cache", host: "127.0.0.1", port: 6379,
  username: null, dbIndex: 0, tls: false, allowWrites: false, hasPassword: false, revision: 1,
};

it("switches Redis → SQL → Redis and submits only the current Redis settings", async () => {
  vi.mocked(commands.saveRedisConnection).mockResolvedValue(savedRedis);
  mount();
  await openConnectionEditor();
  choose("redis");
  enter("Name", "Cache");
  enter("Port", "6380");
  enter("Logical database", "7");
  enter("Password", "redis-secret");
  fireEvent.click(screen.getByLabelText("Allow manual Redis writes on this device"));
  choose("sqlite");
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("Cache");
  expect(screen.getByLabelText("SQLite Path")).toHaveValue("");
  enter("SQLite Path", "example.sqlite");
  choose("redis");
  expect(typeSelect()).toHaveValue("redis");
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("Cache");
  expect(screen.getByLabelText("Port")).toHaveValue(6379);
  expect(screen.getByLabelText("Logical database")).toHaveValue(0);
  expect(passwordInput()).toHaveValue("");
  expect(screen.getByLabelText("Allow manual Redis writes on this device")).not.toBeChecked();
  expectNoConnectionCommands();
  fireEvent.click(screen.getByRole("button", { name: "Save", exact: true }));
  await waitFor(() => expect(commands.saveRedisConnection).toHaveBeenCalledWith({
    workspaceId: "ws", name: "Cache", host: "127.0.0.1", port: 6379, dbIndex: 0,
    username: null, tls: false, allowWrites: false, sshConnectionId: null,
  }, null));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});

it("discards switched drafts on Cancel and Escape", async () => {
  mount();
  await openConnectionEditor();
  choose("redis");
  enter("Name", "Cancelled cache");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await openConnectionEditor();
  expect(typeSelect()).toHaveValue("sqlite");
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("");
  choose("mysql");
  enter("Password", "discard me");
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  await openConnectionEditor();
  choose("mysql");
  expect(passwordInput()).toHaveValue("");
  expectNoConnectionCommands();
});

it("locks the type while saving Redis and keeps the next workspace's editor open", async () => {
  const save = deferred<commands.RedisConnection>();
  vi.mocked(commands.saveRedisConnection).mockReturnValueOnce(save.promise);
  const { showWorkspace } = mount();
  await openConnectionEditor();
  enter("Name", "Cache");
  choose("redis");
  fireEvent.click(screen.getByRole("button", { name: "Save", exact: true }));
  expect(typeSelect()).toBeDisabled();
  choose("postgres");
  expect(typeSelect()).toHaveValue("redis");
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(screen.getByRole("dialog")).toBeVisible();
  showWorkspace("other");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await openConnectionEditor();
  enter("Name", "Other workspace");
  await act(async () => save.resolve(savedRedis));
  expect(typeSelect()).toHaveValue("sqlite");
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("Other workspace");
  expect(useDatabaseTabStore.getState().byWorkspace.ws?.tabs ?? []).not.toContainEqual(expect.objectContaining({ kind: "redis" }));
  expect(commands.saveRedisConnection).toHaveBeenCalledTimes(1);
  expect(commands.saveDatabaseConnection).not.toHaveBeenCalled();
});

const types = ["sqlite", "redis"] as const;
type ConnectionType = typeof types[number];
const testButton = (name = "Test connection") => screen.getByRole("button", { name });
const saveButton = () => screen.getByRole("button", { name: "Save", exact: true });
const cancelButton = () => screen.getByRole("button", { name: "Cancel" });
const formElement = () => saveButton().closest("form")!;

async function openValidDraft(type: ConnectionType, name = "Test connection draft") {
  await openConnectionEditor();
  choose(type);
  enter("Name", name);
  if (type === "sqlite") enter("SQLite Path", "example.sqlite");
}

function deferConnectionTest(type: ConnectionType) {
  const test = deferred<commands.DatabaseTestResult>();
  const command = type === "sqlite"
    ? vi.mocked(commands.testDatabaseConnectionInput).mockReturnValueOnce(test.promise)
    : vi.mocked(commands.testRedisConnectionInput).mockReturnValueOnce(test.promise.then(() => undefined));
  return { command, ...test, succeed: () => test.resolve({ ok: true, message: "Connection OK", serverVersion: null }) };
}

function expectLocked(testLabel: string) {
  const dialog = screen.getByRole("dialog");
  for (const control of formElement().querySelectorAll("input, select, button")) {
    expect(control).toBeDisabled();
  }
  fireEvent.click(testButton(testLabel));
  fireEvent.click(saveButton());
  fireEvent.submit(formElement());
  fireEvent.click(cancelButton());
  fireEvent.keyDown(dialog, { key: "Escape" });
  const overlay = document.querySelector(".u-dialog-overlay")!;
  fireEvent.pointerDown(overlay, { pointerType: "mouse", button: 0 });
  fireEvent.click(overlay);
  expect(dialog).toBeVisible();
}

async function expectSuccessfulTest(type: ConnectionType) {
  expect((await screen.findAllByText("Connection succeeded"))[0]).toBeVisible();
  if (type === "sqlite") fireEvent.click(screen.getByRole("button", { name: "Close", exact: true }));
  expect(testButton()).toBeEnabled();
  expect(saveButton()).toBeEnabled();
  expect(typeSelect()).toBeEnabled();
  expect(cancelButton()).toBeEnabled();
}

it.each(types)("locks the %s form during testing and saving and dispatches each action once", async (type) => {
  const test = deferConnectionTest(type);
  const save = deferred<void>();
  const saveCommand = type === "sqlite"
    ? vi.mocked(commands.saveDatabaseConnection).mockReturnValueOnce(save.promise.then(() => connection("saved", "sqlite")))
    : vi.mocked(commands.saveRedisConnection).mockReturnValueOnce(save.promise.then(() => savedRedis));
  mount();
  await openValidDraft(type);
  const testAction = testButton();
  const form = formElement();
  act(() => {
    fireEvent.click(testAction);
    fireEvent.click(testAction);
    fireEvent.submit(form);
  });
  await waitFor(() => expect(testButton("Testing…")).toBeDisabled());
  expectLocked("Testing…");
  expect(test.command).toHaveBeenCalledTimes(1);
  expect(saveCommand).not.toHaveBeenCalled();

  await act(async () => test.succeed());
  await expectSuccessfulTest(type);
  act(() => {
    fireEvent.submit(form);
    fireEvent.submit(form);
    fireEvent.click(testButton());
  });
  await waitFor(() => expect(saveButton()).toBeDisabled());
  expectLocked("Test connection");
  expect(saveCommand).toHaveBeenCalledTimes(1);
  expect(test.command).toHaveBeenCalledTimes(1);

  await act(async () => save.resolve());
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});

it.each(types)("allows edits and another %s test after a failure", async (type) => {
  const failed = deferConnectionTest(type);
  const retry = deferConnectionTest(type);
  mount();
  await openValidDraft(type);
  fireEvent.click(testButton());
  await act(async () => failed.reject(new Error("Endpoint unavailable")));
  if (type === "sqlite") {
    expect(await screen.findByText("Endpoint unavailable")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Close", exact: true }));
  } else {
    expect(await screen.findByRole("alert")).toHaveTextContent("Redis operation failed.");
  }

  const label = type === "sqlite" ? "SQLite Path" : "Host";
  expect(screen.getByLabelText(label)).toBeEnabled();
  enter(label, type === "sqlite" ? "updated.sqlite" : "updated.example.test");
  fireEvent.click(testButton());
  await waitFor(() => expect(retry.command).toHaveBeenLastCalledWith(
    expect.objectContaining(type === "sqlite" ? { sqlitePath: "updated.sqlite" } : { host: "updated.example.test" }), null,
  ));
  await act(async () => retry.succeed());
  await expectSuccessfulTest(type);
});

it.each(types)("keeps a new %s test pending when the old editor's test completes", async (type) => {
  const oldTest = deferConnectionTest(type);
  const newTest = deferConnectionTest(type);
  const { showWorkspace } = mount();
  await openValidDraft(type, "Old draft");
  fireEvent.click(testButton());
  await waitFor(() => expect(oldTest.command).toHaveBeenCalledTimes(1));

  showWorkspace("other");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  showWorkspace("ws");
  await openValidDraft(type, "New draft");
  expect(testButton()).toBeEnabled();
  fireEvent.click(testButton());
  await waitFor(() => expect(newTest.command).toHaveBeenCalledTimes(2));
  await act(async () => oldTest.succeed());
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.queryByText("Connection succeeded")).not.toBeInTheDocument();
  expect(testButton("Testing…")).toBeDisabled();
  expect(screen.getByLabelText("Name", { exact: true })).toHaveValue("New draft");
  await act(async () => newTest.succeed());
  await expectSuccessfulTest(type);
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
